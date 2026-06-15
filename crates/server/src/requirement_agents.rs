//! Shared helpers for requirement lifecycle features that delegate work to a code
//! agent executed by a suitable employee.
//!
//! Every lifecycle enhancement (optimize requirement, split development tasks,
//! split / execute test tasks, package / start / inspect a release, continue or
//! review a development task) follows the same shape:
//!
//! 1. pick a suitable employee for the relevant role,
//! 2. spawn a streaming code-agent run mirrored into the employee conversation
//!    (so the chat panel renders progress in real time),
//! 3. optionally reconcile filesystem artifacts back into work tasks once the
//!    agent finishes.
//!
//! This module centralizes the employee selection and background-spawn plumbing
//! so each feature module only has to build a prompt and a post-completion hook.

use crate::{
    conversation_task::run_with_conversation,
    employee::{list_employee_records, EmployeeRecord},
    tasks::{AgentTaskRecord, CodeAgentTaskParams, TaskKind, TaskRunner},
    tools::{driver::ToolChatMessage, manager::ToolManager},
    work_rules::{load_work_rules, resolve_role_key},
};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Response returned by endpoints that dispatch a background code-agent run,
/// telling the UI which employee was assigned so it can surface progress.
#[derive(Debug, Clone, Serialize)]
pub struct AgentDispatchWire {
    pub employee_id: String,
    pub employee_name: String,
    pub role: String,
}

impl AgentDispatchWire {
    pub fn from_employee(employee: &EmployeeRecord) -> Self {
        Self {
            employee_id: employee.id.clone(),
            employee_name: employee.name.clone(),
            role: employee.role.clone(),
        }
    }
}

/// Bundles everything needed to launch a single code-agent run.
pub struct AgentTaskSpec {
    pub kind: TaskKind,
    pub content: String,
    pub workdir: PathBuf,
    pub messages: Vec<ToolChatMessage>,
    pub context: Value,
}

/// Picks the most suitable employee for a given work-rules role key (for example
/// `product`, `engineering`, `testing`, `operations`).
///
/// Resolution order:
/// 1. an employee whose role resolves to `role_key` via the work rules,
/// 2. otherwise the first available employee (so the feature still works even
///    when role metadata is missing).
///
/// Returns `None` only when the company has no employees at all.
pub fn pick_employee_for_role(workspace: &Path, role_key: &str) -> Option<EmployeeRecord> {
    let employees = list_employee_records(workspace).ok()?;
    if employees.is_empty() {
        return None;
    }
    if let Ok(rules) = load_work_rules(workspace) {
        if let Some(found) = employees
            .iter()
            .find(|e| resolve_role_key(&rules, &e.role).as_deref() == Some(role_key))
        {
            return Some(found.clone());
        }
    }
    employees.into_iter().next()
}

async fn run_agent_streaming(
    workspace: &Path,
    tools: &ToolManager,
    employee_id: &str,
    spec: AgentTaskSpec,
    event_tx: tokio::sync::mpsc::Sender<crate::tools::driver::ChatStreamEvent>,
) -> anyhow::Result<AgentTaskRecord> {
    let params = CodeAgentTaskParams {
        kind: spec.kind,
        content: spec.content,
        workdir: spec.workdir,
        messages: spec.messages,
        executor_id: Some(employee_id.to_string()),
        parent_task_id: None,
        context: spec.context,
    };
    let runner = TaskRunner::new(workspace);
    let (task, _instance, _result, _events) = runner
        .run_code_chat_streaming_events(tools, params, event_tx)
        .await?;
    Ok(task)
}

/// Spawns a background streaming code-agent run for `employee_id`, mirrored into
/// their conversation, and invokes `on_complete` with the workspace path once the
/// run finishes successfully.
///
/// The agent run is long-lived (it spawns an external coding CLI), so it MUST run
/// off the request path. Callers should clone the [`ToolManager`] before invoking
/// this to release the tools lock immediately.
pub fn spawn_requirement_agent_task<F>(
    workspace: &Path,
    tools: &ToolManager,
    employee_id: &str,
    spec: AgentTaskSpec,
    on_complete: F,
) where
    F: FnOnce(&Path) + Send + 'static,
{
    let ws = workspace.to_path_buf();
    let tools = tools.clone();
    let employee_id = employee_id.to_string();
    tokio::spawn(async move {
        let ws_inner = ws.clone();
        let emp_inner = employee_id.clone();
        let res = run_with_conversation(&ws, &employee_id, move |tx| async move {
            run_agent_streaming(&ws_inner, &tools, &emp_inner, spec, tx).await
        })
        .await;
        match res {
            Ok(_) => on_complete(&ws),
            Err(err) => {
                tracing::warn!(error = %err, "requirement agent task failed");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_workspace() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-req-agents-{unique}"))
    }

    fn write_employee(workspace: &Path, id: &str, role: &str) {
        let dir = crate::employee::employee_root(workspace).join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("profile.json"),
            serde_json::json!({
                "id": id,
                "name": id,
                "department": "x",
                "role": role,
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn picks_employee_matching_role_key() {
        let workspace = temp_workspace();
        write_employee(&workspace, "pat", "Product Manager");
        write_employee(&workspace, "ed", "Engineer");
        let picked = pick_employee_for_role(&workspace, "engineering").unwrap();
        assert_eq!(picked.id, "ed");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn falls_back_to_first_employee_when_no_role_match() {
        let workspace = temp_workspace();
        write_employee(&workspace, "ann", "Designer");
        let picked = pick_employee_for_role(&workspace, "testing").unwrap();
        assert_eq!(picked.id, "ann");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn returns_none_without_employees() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        assert!(pick_employee_for_role(&workspace, "product").is_none());
        let _ = fs::remove_dir_all(&workspace);
    }
}
