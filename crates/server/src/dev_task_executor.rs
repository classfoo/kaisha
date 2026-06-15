use crate::git::{repo_dir, MAIN_REPO_ID};
use crate::tasks::{
    AgentTaskRecord, CodeAgentTaskParams, TaskKind, TaskRunner,
};
use crate::tools::driver::{ChatStreamEvent, ToolChatMessage};
use crate::tools::manager::ToolManager;
use crate::work_task::{load_work_task, task_branch, update_work_task};
use std::path::{Path, PathBuf};

/// Returns the git repository path for development task execution.
///
/// The code agent will use this directory as its working directory (cwd) when
/// executing development tasks, ensuring all file operations target the correct
/// git-managed project.
pub fn dev_task_workdir(workspace: &Path) -> PathBuf {
    repo_dir(workspace, MAIN_REPO_ID)
}

/// Builds the prompt content for a development task.
///
/// Includes task title, description, branch information, and execution instructions.
pub fn build_dev_task_prompt(
    workspace: &Path,
    task_id: &str,
    requirement_id: &str,
) -> Result<String, String> {
    let task = load_work_task(workspace, task_id)?;

    let title = &task.title;
    let description = &task.description;
    let branch = task_branch(&task).unwrap_or("unknown");

    let prompt = format!(
        r#"You are working on a development task.

## Task Information

**Task ID:** {task_id}
**Task Title:** {title}
**Requirement ID:** {requirement_id}
**Feature Branch:** {branch}

## Task Description

{description}

## Instructions

1. Read and understand the task description above.
2. Check out the feature branch `{branch}` if not already on it.
3. Implement the required changes in the codebase.
4. Ensure all changes are well-structured and follow existing code patterns.
5. Commit your changes with a descriptive commit message.

**Important:** All file operations must be performed relative to this repository root.
Do not modify files outside of the intended scope of this task."#
    );

    Ok(prompt)
}

/// Executes a development task using the code agent, streaming its progress into
/// the assignee's conversation so the employee chat panel renders the process in
/// real time.
///
/// This function:
/// 1. Resolves the git repository as the working directory
/// 2. Builds the task prompt from task metadata
/// 3. Invokes the code agent (streaming) bridged to the employee conversation
/// 4. Links the resulting agent task to the work task and returns its record
pub async fn execute_dev_task_streaming(
    workspace: &Path,
    tools: &ToolManager,
    task_id: &str,
    requirement_id: &str,
    employee_id: &str,
) -> anyhow::Result<AgentTaskRecord> {
    let ws_for_runner = workspace.to_path_buf();
    let tools = tools.clone();
    let task_id = task_id.to_string();
    let requirement_id = requirement_id.to_string();
    let employee_id_owned = employee_id.to_string();
    crate::conversation_task::run_with_conversation(
        workspace,
        employee_id,
        move |tx| async move {
            run_dev_task_agent_streaming(
                &ws_for_runner,
                &tools,
                &task_id,
                &requirement_id,
                &employee_id_owned,
                tx,
            )
            .await
        },
    )
    .await
}

/// Runs the development task code agent, streaming its progress through the
/// caller-provided `event_tx` channel without managing any conversation state.
///
/// This is the shared primitive used both by [`execute_dev_task_streaming`]
/// (which wraps it in conversation bookkeeping) and by the autonomy exploration
/// flow (which streams into an already-open `task_process` message). It builds the
/// task prompt, executes the code agent in the git repository working directory,
/// links the resulting agent task to the work task, and returns the record.
pub async fn run_dev_task_agent_streaming(
    workspace: &Path,
    tools: &ToolManager,
    task_id: &str,
    requirement_id: &str,
    employee_id: &str,
    event_tx: tokio::sync::mpsc::Sender<ChatStreamEvent>,
) -> anyhow::Result<AgentTaskRecord> {
    let workdir = dev_task_workdir(workspace);
    let prompt = build_dev_task_prompt(workspace, task_id, requirement_id)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let messages = vec![ToolChatMessage {
        role: "user".into(),
        content: prompt.clone(),
    }];

    let params = CodeAgentTaskParams {
        kind: TaskKind::WorkTaskExecute,
        content: prompt,
        workdir,
        messages,
        executor_id: Some(employee_id.to_string()),
        parent_task_id: None,
        context: serde_json::json!({
            "employee_id": employee_id,
            "task_id": task_id,
            "requirement_id": requirement_id,
        }),
    };

    let runner = TaskRunner::new(workspace);
    let (task, _instance, _result, _events) = runner
        .run_code_chat_streaming_events(tools, params, event_tx)
        .await?;

    // Link agent task to work task
    let _ = link_agent_task_to_work_task(workspace, task_id, &task.id);

    Ok(task)
}

/// Builds the prompt content for reviewing the code produced for a development task.
pub fn build_dev_task_review_prompt(
    workspace: &Path,
    task_id: &str,
    requirement_id: &str,
) -> Result<String, String> {
    let task = load_work_task(workspace, task_id)?;
    let title = &task.title;
    let description = &task.description;
    let branch = task_branch(&task).unwrap_or("unknown");

    let prompt = format!(
        r#"You are reviewing the code produced for a development task.

## Task Information

**Task ID:** {task_id}
**Task Title:** {title}
**Requirement ID:** {requirement_id}
**Feature Branch:** {branch}

## Task Description

{description}

## Instructions

1. Check out the feature branch `{branch}` if not already on it.
2. Inspect the changes on this branch (for example `git log` and `git diff` against the base branch).
3. Review the code for correctness, completeness against the task description, code quality, tests and potential regressions.
4. If you find small, safe issues you may fix them directly and commit. For larger concerns, clearly list them.
5. Reply with a concise review: what looks good, what needs change, and an overall verdict (approve / needs change).

**Important:** All file operations must be performed relative to this repository root."#
    );

    Ok(prompt)
}

/// Executes a code review for a development task using the code agent, streaming
/// progress into the assignee's conversation.
pub async fn execute_dev_task_review_streaming(
    workspace: &Path,
    tools: &ToolManager,
    task_id: &str,
    requirement_id: &str,
    employee_id: &str,
) -> anyhow::Result<AgentTaskRecord> {
    let ws_for_runner = workspace.to_path_buf();
    let tools = tools.clone();
    let task_id = task_id.to_string();
    let requirement_id = requirement_id.to_string();
    let employee_id_owned = employee_id.to_string();
    crate::conversation_task::run_with_conversation(
        workspace,
        employee_id,
        move |tx| async move {
            let workdir = dev_task_workdir(&ws_for_runner);
            let prompt = build_dev_task_review_prompt(&ws_for_runner, &task_id, &requirement_id)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let messages = vec![ToolChatMessage {
                role: "user".into(),
                content: prompt.clone(),
            }];
            let params = CodeAgentTaskParams {
                kind: TaskKind::WorkTaskExecute,
                content: prompt,
                workdir,
                messages,
                executor_id: Some(employee_id_owned.clone()),
                parent_task_id: None,
                context: serde_json::json!({
                    "employee_id": employee_id_owned,
                    "task_id": task_id,
                    "requirement_id": requirement_id,
                    "review": true,
                }),
            };
            let runner = TaskRunner::new(&ws_for_runner);
            let (task, _instance, _result, _events) = runner
                .run_code_chat_streaming_events(&tools, params, tx)
                .await?;
            let _ = link_agent_task_to_work_task(&ws_for_runner, &task_id, &task.id);
            Ok(task)
        },
    )
    .await
}

/// Links an agent task to a work task for tracking purposes.
fn link_agent_task_to_work_task(
    workspace: &Path,
    work_task_id: &str,
    agent_task_id: &str,
) -> Result<(), String> {
    update_work_task(workspace, work_task_id, |task| {
        task.agent_task_id = Some(agent_task_id.to_string());
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work_task::{
        create_work_task, CreateWorkTaskParams, TASK_KIND_DEVELOPMENT,
    };
    use std::{fs, time::{SystemTime, UNIX_EPOCH}};

    fn temp_workspace() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-dev-executor-test-{unique}"))
    }

    #[test]
    fn dev_task_workdir_resolves_to_main_repo() {
        let workspace = temp_workspace();
        let git_repo = workspace.join("repos").join("main");
        fs::create_dir_all(&git_repo).unwrap();
        std::process::Command::new("git")
            .current_dir(&git_repo)
            .args(["init"])
            .output()
            .unwrap();

        let workdir = dev_task_workdir(&workspace);
        assert_eq!(
            workdir,
            workspace.join("repos").join("main")
        );
        assert!(workdir.join(".git").exists());

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn build_dev_task_prompt_includes_task_info() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();

        let task = create_work_task(
            &workspace,
            CreateWorkTaskParams {
                id: Some("test-task-001"),
                biz_type: "requirement",
                biz_id: "test-req",
                title: "Implement login feature",
                description: "Add OAuth2 login endpoint",
                assignee: Some("alice"),
                auto_executable: true,
                metadata: serde_json::json!({
                    "task_kind": TASK_KIND_DEVELOPMENT,
                    "branch": "feat-login-001",
                    "dev_status": "branch_created",
                }),
            },
        )
        .unwrap();

        let prompt = build_dev_task_prompt(&workspace, &task.id, "test-req").unwrap();
        assert!(prompt.contains("Implement login feature"));
        assert!(prompt.contains("Add OAuth2 login endpoint"));
        assert!(prompt.contains("feat-login-001"));
        assert!(prompt.contains("test-req"));

        let _ = fs::remove_dir_all(&workspace);
    }
}
