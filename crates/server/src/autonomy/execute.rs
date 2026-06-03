use crate::employee_todo::{self, EmployeeTodoFile};
use crate::i18n;
use crate::tasks::{AgentTaskRecord, CodeAgentTaskParams, TaskKind, TaskRunner};
use crate::tools::driver::{ChatStreamEvent, ToolChatMessage};
use crate::tools::manager::ToolManager;
use axum::http::HeaderMap;
use std::path::{Path, PathBuf};

/// Runs an autonomy execute for the given employee.
/// This invokes the LLM agent to execute existing todos.
pub fn run_execute(
    workspace: &Path,
    tools: &ToolManager,
    employee_id: &str,
    headers: &HeaderMap,
) -> anyhow::Result<EmployeeTodoFile> {
    let lang = i18n::resolve_lang(headers);

    let system_prompt = build_system_prompt(headers, &lang);
    let user_context = build_user_context(workspace, employee_id, headers, &lang);

    let messages = vec![
        ToolChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        ToolChatMessage {
            role: "user".to_string(),
            content: user_context,
        },
    ];

    let content = i18n::format_msg(
        &lang,
        "task_content_autonomy_execute",
        &[("employee_id", employee_id)],
    );

    let workdir = employee_todo::todo_path(workspace, employee_id)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.to_path_buf());

    let params = CodeAgentTaskParams {
        kind: TaskKind::AutonomyExecute,
        content,
        workdir,
        messages,
        executor_id: Some(employee_id.to_string()),
        parent_task_id: None,
        context: serde_json::json!({
            "employee_id": employee_id,
            "plan_trigger": "manual",
        }),
    };

    let runner = TaskRunner::new(workspace);
    let _ = runner.run_code_chat(tools, params);
    employee_todo::load_todos(workspace, employee_id)
}

/// Runs an autonomy execute with streaming events.
/// Invokes the LLM agent and streams execution events through `event_tx`.
pub async fn run_execute_streaming(
    workspace: &Path,
    tools: &ToolManager,
    employee_id: &str,
    headers: &HeaderMap,
    event_tx: tokio::sync::mpsc::Sender<ChatStreamEvent>,
) -> anyhow::Result<AgentTaskRecord> {
    let lang = i18n::resolve_lang(headers);

    let system_prompt = build_system_prompt(headers, &lang);
    let user_context = build_user_context(workspace, employee_id, headers, &lang);

    let messages = vec![
        ToolChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        ToolChatMessage {
            role: "user".to_string(),
            content: user_context,
        },
    ];

    let content = i18n::format_msg(
        &lang,
        "task_content_autonomy_execute",
        &[("employee_id", employee_id)],
    );

    let workdir = employee_todo::todo_path(workspace, employee_id)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.to_path_buf());

    let params = CodeAgentTaskParams {
        kind: TaskKind::AutonomyExecute,
        content,
        workdir,
        messages,
        executor_id: Some(employee_id.to_string()),
        parent_task_id: None,
        context: serde_json::json!({
            "employee_id": employee_id,
            "plan_trigger": "manual",
        }),
    };

    let runner = TaskRunner::new(workspace);
    let (task, _instance, _result, _events) = runner.run_code_chat_streaming_events(tools, params, event_tx).await?;

    // Mark autonomy task as completed/failed in the todo file
    let _ = crate::autonomy_task::sync_work_task_status(
        workspace,
        &task.id,
        &crate::tasks::TaskStatus::Completed,
    );

    Ok(task)
}

fn build_system_prompt(headers: &HeaderMap, lang: &str) -> String {
    let mut parts = Vec::new();

    parts.push(i18n::msg(headers, "autonomy_execute_planner_intro"));
    parts.push(i18n::agent_language_directive(lang));
    parts.push(i18n::msg(
        headers,
        "autonomy_execute_instructions",
    ));

    parts.join("\n\n")
}

fn build_user_context(workspace: &Path, employee_id: &str, headers: &HeaderMap, _lang: &str) -> String {
    let mut sections = Vec::new();

    // Employee profile
    let profile_path = crate::employee::employee_root(workspace)
        .join(employee_id)
        .join("profile.json");
    if profile_path.exists() {
        if let Ok(raw) = std::fs::read_to_string(&profile_path) {
            sections.push(format!("## Employee profile\n```json\n{}\n```", raw));
        }
    }

    // Existing todos to execute
    let todos = employee_todo::load_todos(workspace, employee_id).unwrap_or_else(|_| EmployeeTodoFile {
        employee_id: employee_id.to_string(),
        items: Vec::new(),
        last_autonomy_run_ms: None,
    });
    let todos_section = if todos.items.is_empty() {
        i18n::msg(headers, "autonomy_execute_no_pending_todos")
    } else {
        let pending_items: Vec<_> = todos.items.iter().filter(|item| {
            matches!(item.status, crate::employee_todo::TodoStatus::Pending)
        }).collect();
        if pending_items.is_empty() {
            i18n::msg(headers, "autonomy_execute_no_pending_todos")
        } else {
            let mut lines = Vec::new();
            for item in &pending_items {
                let source = format!("{:?}", item.source);
                lines.push(format!(
                    "- {} (source: {}){}",
                    item.title,
                    source,
                    item.requirement_id
                        .as_ref()
                        .map(|r| format!(" | requirement: {}", r))
                        .unwrap_or_default(),
                ));
            }
            lines.join("\n")
        }
    };
    sections.push(format!(
        "{}\n{}",
        i18n::msg(headers, "autonomy_execute_section_pending_todos"),
        todos_section,
    ));

    // Requirement context
    if let Ok(requirements) = list_requirements(workspace) {
        if !requirements.is_empty() {
            sections.push(format!(
                "{}\n{}",
                i18n::msg(headers, "autonomy_execute_section_requirement_context"),
                requirements,
            ));
        }
    }

    sections.push(i18n::format_msg(
        "en",
        "autonomy_execute_user_prompt",
        &[],
    ));

    sections.join("\n\n")
}

fn list_requirements(workspace: &Path) -> anyhow::Result<String> {
    if let Ok(_root) = crate::requirement::ensure_requirements_root(workspace) {
        let items = crate::requirement::list_requirement_summaries(workspace)?;
        if items.is_empty() {
            return Ok("(no requirements)".to_string());
        }
        let catalog = crate::employee_requirement_agent::format_requirement_catalog(workspace, &items)?;
        return Ok(catalog);
    }
    Ok("(no requirements directory)".to_string())
}
