use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const BIZ_TYPE_REQUIREMENT: &str = "requirement";
pub const TASK_KIND_DEVELOPMENT: &str = "development";
pub const TASK_KIND_REVIEW: &str = "review";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkTaskStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
    Failed,
}

impl WorkTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkTask {
    pub id: String,
    pub biz_type: String,
    pub biz_id: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    pub status: WorkTaskStatus,
    pub progress: u8,
    #[serde(default = "default_auto_executable")]
    pub auto_executable: bool,
    #[serde(default)]
    pub metadata: Value,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_task_id: Option<String>,
}

fn default_auto_executable() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkTaskWire {
    pub id: String,
    pub biz_type: String,
    pub biz_id: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    pub status: String,
    pub progress: u8,
    pub auto_executable: bool,
    pub metadata: Value,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_task_id: Option<String>,
}

pub fn work_tasks_root(workspace: &Path) -> PathBuf {
    workspace.join("work_tasks")
}

fn work_task_path(workspace: &Path, task_id: &str) -> PathBuf {
    work_tasks_root(workspace).join(format!("{task_id}.json"))
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_work_task_id() -> String {
    format!("wtask_{}", now_ms())
}

pub fn wire_task(task: &WorkTask) -> WorkTaskWire {
    WorkTaskWire {
        id: task.id.clone(),
        biz_type: task.biz_type.clone(),
        biz_id: task.biz_id.clone(),
        title: task.title.clone(),
        description: task.description.clone(),
        assignee: task.assignee.clone(),
        status: task.status.as_str().to_string(),
        progress: task.progress,
        auto_executable: task.auto_executable,
        metadata: task.metadata.clone(),
        created_at_ms: task.created_at_ms,
        updated_at_ms: task.updated_at_ms,
        agent_task_id: task.agent_task_id.clone(),
    }
}

pub fn is_development_task(task: &WorkTask) -> bool {
    task.metadata
        .get("task_kind")
        .and_then(|v| v.as_str())
        == Some(TASK_KIND_DEVELOPMENT)
}

pub fn is_review_task(task: &WorkTask) -> bool {
    task.metadata
        .get("task_kind")
        .and_then(|v| v.as_str())
        == Some(TASK_KIND_REVIEW)
}

pub fn review_opinion_status(task: &WorkTask) -> Option<&str> {
    task.metadata
        .get("opinion_status")
        .and_then(|v| v.as_str())
}

pub fn review_passed(task: &WorkTask) -> Option<bool> {
    task.metadata.get("passed").and_then(|v| v.as_bool())
}

pub fn set_review_opinion_status(task: &mut WorkTask, status: &str) {
    if let Value::Object(map) = &mut task.metadata {
        map.insert("opinion_status".into(), Value::String(status.into()));
    } else {
        task.metadata = serde_json::json!({ "opinion_status": status });
    }
}

pub fn set_review_phase(task: &mut WorkTask, phase: &str) {
    if let Value::Object(map) = &mut task.metadata {
        map.insert("review_phase".into(), Value::String(phase.into()));
    } else {
        task.metadata = serde_json::json!({ "review_phase": phase });
    }
}

pub fn set_review_passed(task: &mut WorkTask, passed: Option<bool>) {
    if let Value::Object(map) = &mut task.metadata {
        match passed {
            Some(value) => {
                map.insert("passed".into(), Value::Bool(value));
            }
            None => {
                map.remove("passed");
            }
        }
    }
}

pub fn dev_status(task: &WorkTask) -> Option<&str> {
    task.metadata
        .get("dev_status")
        .and_then(|v| v.as_str())
}

pub fn task_branch(task: &WorkTask) -> Option<&str> {
    task.metadata.get("branch").and_then(|v| v.as_str())
}

pub fn set_dev_status(task: &mut WorkTask, status: &str) {
    if let Value::Object(map) = &mut task.metadata {
        map.insert("dev_status".into(), Value::String(status.into()));
    } else {
        task.metadata = serde_json::json!({ "dev_status": status });
    }
}

/// Sets the agent_task_id on a work task to link it with an agent execution task.
pub fn set_agent_task_id(task: &mut WorkTask, agent_task_id: &str) {
    task.agent_task_id = Some(agent_task_id.to_string());
}

pub fn save_work_task(workspace: &Path, task: &WorkTask) -> Result<(), String> {
    let path = work_task_path(workspace, &task.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(task).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn load_work_task(workspace: &Path, task_id: &str) -> Result<WorkTask, String> {
    let path = work_task_path(workspace, task_id);
    if !path.exists() {
        return Err("work_task_not_found".to_string());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn delete_work_task(workspace: &Path, task_id: &str) -> Result<(), String> {
    let path = work_task_path(workspace, task_id);
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn list_work_tasks(workspace: &Path) -> Result<Vec<WorkTask>, String> {
    let root = work_tasks_root(workspace);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Ok(task) = serde_json::from_str::<WorkTask>(&raw) {
            items.push(task);
        }
    }
    items.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms));
    Ok(items)
}

#[derive(Debug, Clone, Default)]
pub struct WorkTaskFilter {
    pub biz_type: Option<String>,
    pub biz_id: Option<String>,
    pub assignee: Option<String>,
    pub status: Option<WorkTaskStatus>,
    pub auto_executable: Option<bool>,
    pub task_kind: Option<String>,
}

pub fn filter_work_tasks(tasks: Vec<WorkTask>, filter: &WorkTaskFilter) -> Vec<WorkTask> {
    tasks
        .into_iter()
        .filter(|task| {
            if let Some(ref biz_type) = filter.biz_type {
                if task.biz_type != *biz_type {
                    return false;
                }
            }
            if let Some(ref biz_id) = filter.biz_id {
                if task.biz_id != *biz_id {
                    return false;
                }
            }
            if let Some(ref assignee) = filter.assignee {
                if task.assignee.as_deref() != Some(assignee.as_str()) {
                    return false;
                }
            }
            if let Some(status) = filter.status {
                if task.status != status {
                    return false;
                }
            }
            if let Some(auto) = filter.auto_executable {
                if task.auto_executable != auto {
                    return false;
                }
            }
            if let Some(ref kind) = filter.task_kind {
                let task_kind = task
                    .metadata
                    .get("task_kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if task_kind != kind {
                    return false;
                }
            }
            true
        })
        .collect()
}

pub fn list_work_tasks_filtered(
    workspace: &Path,
    filter: &WorkTaskFilter,
) -> Result<Vec<WorkTask>, String> {
    Ok(filter_work_tasks(list_work_tasks(workspace)?, filter))
}

pub struct CreateWorkTaskParams<'a> {
    pub id: Option<&'a str>,
    pub biz_type: &'a str,
    pub biz_id: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub assignee: Option<&'a str>,
    pub auto_executable: bool,
    pub metadata: Value,
}

pub fn create_work_task(workspace: &Path, params: CreateWorkTaskParams<'_>) -> Result<WorkTask, String> {
    let title = params.title.trim();
    if title.is_empty() {
        return Err("task_title_empty".to_string());
    }
    let existing = list_work_tasks_filtered(
        workspace,
        &WorkTaskFilter {
            biz_type: Some(params.biz_type.to_string()),
            biz_id: Some(params.biz_id.to_string()),
            ..Default::default()
        },
    )?;
    if let Some(found) = existing.iter().find(|t| t.title == title) {
        return Ok(found.clone());
    }
    let ts = now_ms();
    let task = WorkTask {
        id: params
            .id
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(new_work_task_id),
        biz_type: params.biz_type.to_string(),
        biz_id: params.biz_id.to_string(),
        title: title.to_string(),
        description: params.description.trim().to_string(),
        assignee: params
            .assignee
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string),
        status: WorkTaskStatus::Pending,
        progress: 0,
        auto_executable: params.auto_executable,
        metadata: params.metadata,
        created_at_ms: ts,
        updated_at_ms: ts,
        agent_task_id: None,
    };
    save_work_task(workspace, &task)?;
    Ok(task)
}

pub fn update_work_task(
    workspace: &Path,
    task_id: &str,
    mut updater: impl FnMut(&mut WorkTask) -> Result<(), String>,
) -> Result<WorkTask, String> {
    let mut task = load_work_task(workspace, task_id)?;
    updater(&mut task)?;
    task.updated_at_ms = now_ms();
    save_work_task(workspace, &task)?;
    Ok(task)
}

use crate::{i18n, AppState};
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::HeaderMap,
    Json,
};

#[derive(Debug, Deserialize)]
pub struct ListWorkTasksQuery {
    pub biz_type: Option<String>,
    pub biz_id: Option<String>,
    pub assignee: Option<String>,
    pub status: Option<WorkTaskStatus>,
    pub auto_executable: Option<bool>,
    pub task_kind: Option<String>,
}

fn workspace_root(state: &AppState) -> Option<PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

pub async fn list_work_tasks_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<ListWorkTasksQuery>,
) -> Result<Json<Vec<WorkTaskWire>>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let filter = WorkTaskFilter {
        biz_type: query.biz_type,
        biz_id: query.biz_id,
        assignee: query.assignee,
        status: query.status,
        auto_executable: query.auto_executable,
        task_kind: query.task_kind,
    };
    let tasks = list_work_tasks_filtered(&workspace, &filter).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    Ok(Json(tasks.iter().map(wire_task).collect()))
}

pub async fn get_work_task_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<WorkTaskWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    load_work_task(&workspace, &task_id)
        .map(|task| Json(wire_task(&task)))
        .map_err(|err| {
            if err == "work_task_not_found" {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    i18n::msg(&headers, "work_task_not_found"),
                )
            } else {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    err,
                )
            }
        })
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
        std::env::temp_dir().join(format!("kaisha-work-task-{unique}"))
    }

    #[test]
    fn create_work_task_with_biz_type_and_id() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let task = create_work_task(
            &workspace,
            CreateWorkTaskParams {
                id: None,
                biz_type: BIZ_TYPE_REQUIREMENT,
                biz_id: "auth",
                title: "Implement login",
                description: "Add login endpoint",
                assignee: Some("alice"),
                auto_executable: true,
                metadata: serde_json::json!({
                    "task_kind": TASK_KIND_DEVELOPMENT,
                    "branch": "feat-auth-wtask_1",
                    "dev_status": "branch_created"
                }),
            },
        )
        .unwrap();
        assert_eq!(task.biz_type, BIZ_TYPE_REQUIREMENT);
        assert_eq!(task.biz_id, "auth");
        assert_eq!(task.assignee.as_deref(), Some("alice"));
        assert!(task.auto_executable);
        assert!(is_development_task(&task));
        let loaded = load_work_task(&workspace, &task.id).unwrap();
        assert_eq!(loaded.title, "Implement login");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn filter_by_biz_and_assignee() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        create_work_task(
            &workspace,
            CreateWorkTaskParams {
                id: None,
                biz_type: BIZ_TYPE_REQUIREMENT,
                biz_id: "auth",
                title: "Task A",
                description: "a",
                assignee: Some("alice"),
                auto_executable: true,
                metadata: serde_json::json!({}),
            },
        )
        .unwrap();
        create_work_task(
            &workspace,
            CreateWorkTaskParams {
                id: None,
                biz_type: BIZ_TYPE_REQUIREMENT,
                biz_id: "auth",
                title: "Task B",
                description: "b",
                assignee: Some("bob"),
                auto_executable: true,
                metadata: serde_json::json!({}),
            },
        )
        .unwrap();
        let alice_tasks = list_work_tasks_filtered(
            &workspace,
            &WorkTaskFilter {
                biz_type: Some(BIZ_TYPE_REQUIREMENT.into()),
                biz_id: Some("auth".into()),
                assignee: Some("alice".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(alice_tasks.len(), 1);
        assert_eq!(alice_tasks[0].title, "Task A");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn duplicate_title_returns_existing_task() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let first = create_work_task(
            &workspace,
            CreateWorkTaskParams {
                id: None,
                biz_type: BIZ_TYPE_REQUIREMENT,
                biz_id: "auth",
                title: "Same title",
                description: "a",
                assignee: None,
                auto_executable: true,
                metadata: serde_json::json!({}),
            },
        )
        .unwrap();
        let second = create_work_task(
            &workspace,
            CreateWorkTaskParams {
                id: None,
                biz_type: BIZ_TYPE_REQUIREMENT,
                biz_id: "auth",
                title: "Same title",
                description: "b",
                assignee: None,
                auto_executable: true,
                metadata: serde_json::json!({}),
            },
        )
        .unwrap();
        assert_eq!(first.id, second.id);
        let _ = fs::remove_dir_all(&workspace);
    }
}
