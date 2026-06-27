//! Requirement testing phase: split a requirement into test tasks and execute
//! them with a code agent driven by a suitable testing employee.
//!
//! Test tasks are stored as Markdown files under
//! `requirements/<id>/testing/tasks/<task-id>.md` and mirrored into [`WorkTask`]
//! records (`task_kind = "test"`), matching the development phase model.

use crate::{
    dev_task_executor::dev_task_workdir,
    i18n,
    requirement::{
        load_requirement_detail, normalize_requirement_id, requirement_dir, requirement_file_path,
    },
    requirement_agents::{
        pick_employee_for_role, spawn_requirement_agent_task, AgentDispatchWire, AgentTaskSpec,
    },
    tasks::TaskKind,
    tools::driver::ToolChatMessage,
    work_task::{
        create_work_task, filter_work_tasks, is_test_task, list_work_tasks, load_work_task,
        set_test_status, test_status, update_work_task, CreateWorkTaskParams, WorkTask,
        WorkTaskFilter, WorkTaskStatus, BIZ_TYPE_REQUIREMENT, TASK_KIND_TEST,
    },
    AppState,
};
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    Json,
};
use serde::Serialize;
use std::{fs, path::Path};

const TESTING_DIR: &str = "testing";
const TASKS_DIR: &str = "tasks";

#[derive(Debug, Clone, Serialize)]
pub struct TestTaskWire {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    pub status: String,
    pub progress: u8,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub biz_type: String,
    pub biz_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementTestingWire {
    pub requirement_id: String,
    pub tasks: Vec<TestTaskWire>,
}

fn testing_tasks_dir(workspace: &Path, id: &str) -> std::path::PathBuf {
    requirement_dir(workspace, id)
        .join(TESTING_DIR)
        .join(TASKS_DIR)
}

fn workspace_root(state: &AppState) -> Option<std::path::PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

fn test_status_of(task: &WorkTask) -> &str {
    test_status(task).unwrap_or(match task.status {
        WorkTaskStatus::Completed => "completed",
        WorkTaskStatus::InProgress => "running",
        WorkTaskStatus::Failed => "failed",
        _ => "pending",
    })
}

fn list_test_work_tasks(workspace: &Path, requirement_id: &str) -> Result<Vec<WorkTask>, String> {
    Ok(filter_work_tasks(
        list_work_tasks(workspace)?,
        &WorkTaskFilter {
            biz_type: Some(BIZ_TYPE_REQUIREMENT.to_string()),
            biz_id: Some(requirement_id.to_string()),
            task_kind: Some(TASK_KIND_TEST.to_string()),
            ..Default::default()
        },
    ))
}

/// Scans `testing/tasks/*.md` and creates a [`WorkTask`] per file that does not
/// already have one. Returns silently when there is no testing directory yet.
pub fn reconcile_test_work_tasks(workspace: &Path, requirement_id: &str) -> Result<(), String> {
    let id = normalize_requirement_id(requirement_id).map_err(|e| e.to_string())?;
    let tasks_dir = testing_tasks_dir(workspace, &id);
    if !tasks_dir.exists() {
        return Ok(());
    }
    let existing = list_test_work_tasks(workspace, &id)?;
    let default_assignee = existing
        .iter()
        .find_map(|task| task.assignee.clone())
        .filter(|value| !value.trim().is_empty());

    for entry in fs::read_dir(&tasks_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let task_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| "task_id_invalid".to_string())?
            .to_string();
        if existing.iter().any(|task| task.id == task_id) {
            continue;
        }
        let description = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let title = description
            .lines()
            .find(|line| line.starts_with("# "))
            .map(|line| line.trim_start_matches("# ").trim())
            .filter(|line| !line.is_empty())
            .unwrap_or(&task_id)
            .to_string();
        create_work_task(
            workspace,
            CreateWorkTaskParams {
                id: Some(&task_id),
                biz_type: BIZ_TYPE_REQUIREMENT,
                biz_id: &id,
                title: &title,
                description: &description,
                assignee: default_assignee.as_deref(),
                auto_executable: false,
                metadata: serde_json::json!({
                    "task_kind": TASK_KIND_TEST,
                    "test_status": "pending",
                }),
            },
        )?;
    }
    Ok(())
}

fn wire_test_task(task: &WorkTask) -> TestTaskWire {
    TestTaskWire {
        id: task.id.clone(),
        title: task.title.clone(),
        assignee: task.assignee.clone(),
        status: test_status_of(task).to_string(),
        progress: task.progress,
        created_at_ms: task.created_at_ms,
        updated_at_ms: task.updated_at_ms,
        biz_type: task.biz_type.clone(),
        biz_id: task.biz_id.clone(),
    }
}

pub async fn get_testing(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RequirementTestingWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id)
        .map_err(|_| (axum::http::StatusCode::BAD_REQUEST, i18n::msg(&headers, "requirement_id_invalid")))?;
    if !requirement_file_path(&workspace, &id).exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }
    let _ = reconcile_test_work_tasks(&workspace, &id);
    let tasks = list_test_work_tasks(&workspace, &id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(RequirementTestingWire {
        requirement_id: id,
        tasks: tasks.iter().map(wire_test_task).collect(),
    }))
}

fn build_split_test_messages(
    requirement_id: &str,
    title: &str,
    content: &str,
    existing_tasks: &[WorkTask],
    next_index: usize,
) -> Vec<ToolChatMessage> {
    let mut existing = String::new();
    if existing_tasks.is_empty() {
        existing.push_str("(none yet)\n");
    } else {
        for task in existing_tasks {
            existing.push_str(&format!("- {} — {} [{}]\n", task.id, task.title, test_status_of(task)));
        }
    }
    let system = format!(
        r#"You are a QA engineer breaking a requirement into concrete test tasks.

## Working directory
This directory is the requirement `{requirement_id}` package. The requirement body is in `requirement.md`. Test tasks live as Markdown files under `testing/tasks/`.

## Existing test tasks
{existing}

## Task
1. Read `requirement.md` and the existing test tasks above.
2. Decide which test scenarios are still needed (do NOT duplicate existing tasks). Cover functional cases, edge cases, error handling and any non-functional requirements. Prefer 2-8 focused test tasks.
3. For each new test task, create a Markdown file `testing/tasks/test-NNN.md` where NNN is a zero-padded 3-digit number. Start numbering at {next_index:03} and increment. Do not overwrite existing files.
4. Each test task file MUST start with a level-1 heading containing the test title, for example `# Login with valid credentials`, followed by preconditions, steps and expected results.
5. Reply briefly listing the test task files you created and their titles.

Do not only describe intent — create the files."#,
        requirement_id = requirement_id,
        existing = existing,
        next_index = next_index,
    );
    vec![
        ToolChatMessage {
            role: "system".to_string(),
            content: system,
        },
        ToolChatMessage {
            role: "user".to_string(),
            content: format!(
                "Split requirement **{title}** (`{requirement_id}`) into test tasks now.\n\n---\n\n{content}"
            ),
        },
    ]
}

/// Dispatches a code-agent task that splits the requirement into test tasks.
pub async fn split_test_tasks(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<AgentDispatchWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id)
        .map_err(|_| (axum::http::StatusCode::BAD_REQUEST, i18n::msg(&headers, "requirement_id_invalid")))?;
    if !requirement_file_path(&workspace, &id).exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }
    let detail = load_requirement_detail(&workspace, &id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let Some(employee) = pick_employee_for_role(&workspace, "testing") else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "requirement_no_employees"),
        ));
    };

    let existing = list_test_work_tasks(&workspace, &id).unwrap_or_default();
    let next_index = existing.len() + 1;
    let messages = build_split_test_messages(&id, &detail.title, &detail.content, &existing, next_index);
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workdir = requirement_dir(&workspace, &id);
    let reconcile_id = id.clone();
    let task = spawn_requirement_agent_task(
        &workspace,
        &tools,
        &employee.id,
        AgentTaskSpec {
            kind: TaskKind::RequirementAgent,
            content: format!("Split test tasks for `{id}`"),
            workdir,
            messages,
            context: serde_json::json!({ "requirement_id": id }),
        },
        move |ws| {
            let _ = reconcile_test_work_tasks(ws, &reconcile_id);
        },
    )
    .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(Json(AgentDispatchWire::from_employee_task(&employee, &task)))
}

fn build_execute_test_messages(task: &WorkTask, requirement_id: &str) -> Vec<ToolChatMessage> {
    let prompt = format!(
        r#"You are executing a test task for requirement `{requirement_id}`.

## Test task
**Task ID:** {task_id}
**Title:** {title}

## Test specification
{description}

## Instructions
1. Set up the project as needed inside this repository working directory.
2. Execute the test scenario described above (run the relevant automated tests or perform the verification steps).
3. Report the result clearly: pass or fail, with evidence (command output, observed vs expected behavior).
4. If the test fails, describe the defect precisely so engineering can fix it.

End your reply with a single line `RESULT: pass` or `RESULT: fail`."#,
        requirement_id = requirement_id,
        task_id = task.id,
        title = task.title,
        description = task.description,
    );
    vec![ToolChatMessage {
        role: "user".to_string(),
        content: prompt,
    }]
}

/// Executes a single test task with the code agent (working in the main repo).
pub async fn test_task_action(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath((id, task_id, action)): AxumPath<(String, String, String)>,
    Json(_payload): Json<serde_json::Value>,
) -> Result<Json<RequirementTestingWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id)
        .map_err(|_| (axum::http::StatusCode::BAD_REQUEST, i18n::msg(&headers, "requirement_id_invalid")))?;
    if action != "execute" {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "task_action_unknown"),
        ));
    }
    let task = load_work_task(&workspace, &task_id).map_err(|_| {
        (
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "task_not_found"),
        )
    })?;
    if !is_test_task(&task) || task.biz_id != id {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "task_not_found"),
        ));
    }

    // Assign a testing employee if the task is unassigned.
    let employee_id = match task
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(existing) => existing.to_string(),
        None => {
            let Some(employee) = pick_employee_for_role(&workspace, "testing") else {
                return Err((
                    axum::http::StatusCode::CONFLICT,
                    i18n::msg(&headers, "requirement_no_employees"),
                ));
            };
            employee.id
        }
    };

    let assign_id = employee_id.clone();
    let _ = update_work_task(&workspace, &task_id, |task| {
        if task
            .assignee
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            task.assignee = Some(assign_id.clone());
        }
        task.status = WorkTaskStatus::InProgress;
        set_test_status(task, "running");
        Ok(())
    });

    let messages = build_execute_test_messages(&task, &id);
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workdir = dev_task_workdir(&workspace);
    let complete_task_id = task_id.clone();
    spawn_requirement_agent_task(
        &workspace,
        &tools,
        &employee_id,
        AgentTaskSpec {
            kind: TaskKind::WorkTaskExecute,
            content: format!("Execute test task `{task_id}` for `{id}`"),
            workdir,
            messages,
            context: serde_json::json!({ "requirement_id": id, "task_id": task_id }),
        },
        move |ws| {
            let _ = update_work_task(ws, &complete_task_id, |task| {
                task.status = WorkTaskStatus::Completed;
                task.progress = 100;
                set_test_status(task, "completed");
                Ok(())
            });
        },
    )
    .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let tasks = list_test_work_tasks(&workspace, &id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(RequirementTestingWire {
        requirement_id: id,
        tasks: tasks.iter().map(wire_test_task).collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requirement::{format_requirement_md, RequirementMeta, RequirementPhase, REQUIREMENT_FILE};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-testing-{unique}"))
    }

    fn seed_requirement(workspace: &Path, id: &str) {
        let dir = requirement_dir(workspace, id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(REQUIREMENT_FILE),
            format_requirement_md(
                &RequirementMeta {
                    id: id.into(),
                    title: "Feature".into(),
                    phase: RequirementPhase::Testing,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                },
                "## Scope",
            ),
        )
        .unwrap();
    }

    #[test]
    fn reconcile_creates_work_tasks_from_md_files() {
        let workspace = temp_workspace();
        seed_requirement(&workspace, "auth");
        let tasks_dir = testing_tasks_dir(&workspace, "auth");
        fs::create_dir_all(&tasks_dir).unwrap();
        fs::write(tasks_dir.join("test-001.md"), "# Login works\n\nSteps...").unwrap();

        reconcile_test_work_tasks(&workspace, "auth").unwrap();
        let tasks = list_test_work_tasks(&workspace, "auth").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "test-001");
        assert_eq!(tasks[0].title, "Login works");
        assert!(is_test_task(&tasks[0]));
        assert_eq!(test_status_of(&tasks[0]), "pending");

        let _ = fs::remove_dir_all(&workspace);
    }
}
