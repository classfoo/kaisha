use crate::employee_todo::{self, EmployeeTodoFile};
use crate::i18n;
use crate::tasks::{AgentTaskRecord, CodeAgentTaskParams, TaskKind, TaskRunner};
use crate::tools::driver::{ChatStreamEvent, ToolChatMessage};
use crate::tools::manager::ToolManager;
use axum::http::HeaderMap;
use std::path::{Path, PathBuf};

/// Runs an autonomy exploration for the given employee.
/// This invokes the LLM agent to analyze the workspace and generate actionable todos.
pub fn run_explore(
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
        "task_content_autonomy_explore",
        &[
            ("employee_id", employee_id),
            ("mode", &i18n::msg_by_lang(&lang, "autonomy_explore_mode_label_requirement_planning")),
        ],
    );

    let workdir = employee_todo::todo_path(workspace, employee_id)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.to_path_buf());

    let params = CodeAgentTaskParams {
        kind: TaskKind::AutonomyExplore,
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
    // Run the agent task. Whether it succeeds or fails, return the current todos
    // since the agent may have written new todos via tool usage.
    let _ = runner.run_code_chat(tools, params);
    employee_todo::load_todos(workspace, employee_id)
}

/// Runs an autonomy exploration with streaming events.
/// Invokes the LLM agent and streams execution events through `event_tx`.
pub async fn run_explore_streaming(
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
        "task_content_autonomy_explore",
        &[
            ("employee_id", employee_id),
            ("mode", &i18n::msg_by_lang(&lang, "autonomy_explore_mode_label_requirement_planning")),
        ],
    );

    let workdir = employee_todo::todo_path(workspace, employee_id)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.to_path_buf());

    let params = CodeAgentTaskParams {
        kind: TaskKind::AutonomyExplore,
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

    parts.push(i18n::msg(headers, "autonomy_explore_planner_intro"));
    parts.push(i18n::agent_language_directive(lang));
    parts.push(i18n::msg(
        headers,
        "autonomy_explore_instructions_requirement_planning",
    ));

    parts.join("\n\n")
}

fn build_user_context(workspace: &Path, employee_id: &str, headers: &HeaderMap, lang: &str) -> String {
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

    // Existing todos
    let todos = employee_todo::load_todos(workspace, employee_id).unwrap_or_else(|_| EmployeeTodoFile {
        employee_id: employee_id.to_string(),
        items: Vec::new(),
        last_autonomy_run_ms: None,
    });
    let todos_section = if todos.items.is_empty() {
        i18n::msg(headers, "autonomy_explore_existing_todos_none")
    } else {
        let mut lines = Vec::new();
        for item in &todos.items {
            let status = format!("{:?}", item.status);
            let source = format!("{:?}", item.source);
            lines.push(format!(
                "- [{}] {} (source: {}){}",
                status,
                item.title,
                source,
                item.requirement_id
                    .as_ref()
                    .map(|r| format!(" | requirement: {}", r))
                    .unwrap_or_default(),
            ));
        }
        lines.join("\n")
    };
    sections.push(format!(
        "{}\n{}",
        i18n::msg(headers, "autonomy_explore_section_existing_todos"),
        todos_section,
    ));

    // Requirement context
    if let Ok(requirements) = list_requirements(workspace) {
        if !requirements.is_empty() {
            sections.push(format!(
                "{}\n{}",
                i18n::msg(headers, "autonomy_explore_section_requirement_context"),
                requirements,
            ));
        }
    }

    // Git context
    if let Some(git_info) = gather_git_context(workspace) {
        sections.push(format!(
            "{}\n{}",
            i18n::msg(headers, "autonomy_explore_section_git_context"),
            git_info,
        ));
    }

    sections.push(i18n::format_msg(
        lang,
        "autonomy_explore_user_prompt",
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

fn gather_git_context(workspace: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .current_dir(workspace)
        .args(["status", "--short", "--branch"])
        .output()
        .ok()?;
    if output.status.success() {
        let status = String::from_utf8_lossy(&output.stdout);
        if !status.trim().is_empty() {
            return Some(format!("```\n{}\n```", status.trim()));
        }
    }
    None
}
