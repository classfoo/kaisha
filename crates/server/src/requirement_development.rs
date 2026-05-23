use crate::{
    i18n,
    requirement::{
        load_requirement_detail, normalize_requirement_id, requirement_dir, RequirementPhase,
    },
    AppState,
};
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const DEV_DIR: &str = "development";
const STATE_FILE: &str = "state.json";
const TASKS_DIR: &str = "tasks";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DevTaskStatus {
    BranchCreated,
    InDevelopment,
    DevComplete,
    InReview,
    ReviewComplete,
    Merged,
}

impl DevTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BranchCreated => "branch_created",
            Self::InDevelopment => "in_development",
            Self::DevComplete => "dev_complete",
            Self::InReview => "in_review",
            Self::ReviewComplete => "review_complete",
            Self::Merged => "merged",
        }
    }

    #[allow(dead_code)]
    pub fn next_status(&self) -> Option<Self> {
        match self {
            Self::BranchCreated => Some(Self::InDevelopment),
            Self::InDevelopment => Some(Self::DevComplete),
            Self::DevComplete => Some(Self::InReview),
            Self::InReview => Some(Self::ReviewComplete),
            Self::ReviewComplete => Some(Self::Merged),
            Self::Merged => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevTask {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    pub branch: String,
    pub status: DevTaskStatus,
    #[serde(default)]
    pub progress: u8,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevStateFile {
    pub requirement_id: String,
    pub feature_branch: String,
    #[serde(default)]
    pub feature_branch_created: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<DevTask>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DevTaskWire {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    pub branch: String,
    pub status: String,
    pub progress: u8,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementDevelopmentWire {
    pub requirement_id: String,
    pub feature_branch: String,
    pub feature_branch_created: bool,
    pub tasks: Vec<DevTaskWire>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_task_id: Option<String>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn dev_dir(workspace: &Path, id: &str) -> std::path::PathBuf {
    requirement_dir(workspace, id).join(DEV_DIR)
}

fn dev_state_path(workspace: &Path, id: &str) -> std::path::PathBuf {
    dev_dir(workspace, id).join(STATE_FILE)
}

fn dev_tasks_dir(workspace: &Path, id: &str) -> std::path::PathBuf {
    requirement_dir(workspace, id).join(DEV_DIR).join(TASKS_DIR)
}

fn task_file_path(workspace: &Path, id: &str, task_id: &str) -> std::path::PathBuf {
    dev_tasks_dir(workspace, id).join(format!("{task_id}.md"))
}

fn load_dev_state(workspace: &Path, id: &str) -> Result<DevStateFile, String> {
    let path = dev_state_path(workspace, id);
    if !path.exists() {
        return Err("development_not_started".to_string());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_dev_state(workspace: &Path, id: &str, state: &DevStateFile) -> Result<(), String> {
    let path = dev_state_path(workspace, id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, serde_json::to_string_pretty(state).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn save_task_content(
    workspace: &Path,
    id: &str,
    task_id: &str,
    content: &str,
) -> Result<(), String> {
    let path = task_file_path(workspace, id, task_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content).map_err(|e| e.to_string())
}

fn wire_task(task: &DevTask) -> DevTaskWire {
    DevTaskWire {
        id: task.id.clone(),
        title: task.title.clone(),
        assignee: task.assignee.clone(),
        branch: task.branch.clone(),
        status: task.status.as_str().to_string(),
        progress: task.progress,
        created_at_ms: task.created_at_ms,
        updated_at_ms: task.updated_at_ms,
    }
}

fn wire_state(state: &DevStateFile) -> RequirementDevelopmentWire {
    RequirementDevelopmentWire {
        requirement_id: state.requirement_id.clone(),
        feature_branch: state.feature_branch.clone(),
        feature_branch_created: state.feature_branch_created,
        tasks: state.tasks.iter().map(wire_task).collect(),
        current_task_id: state.current_task_id.clone(),
    }
}

fn workspace_root(state: &AppState) -> Option<std::path::PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

pub async fn get_development(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RequirementDevelopmentWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_dev_err(&headers))?;
    let file_path = crate::requirement::requirement_file_path(&workspace, &id);
    if !file_path.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }
    let dev_dir_path = dev_dir(&workspace, &id);
    if !dev_dir_path.exists() {
        return Ok(Json(RequirementDevelopmentWire {
            requirement_id: id,
            feature_branch: String::new(),
            feature_branch_created: false,
            tasks: vec![],
            current_task_id: None,
        }));
    }
    let state = load_dev_state(&workspace, &id).map_err(|e| {
        if e == "development_not_started" {
            (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(&headers, "development_not_started"),
            )
        } else {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        }
    })?;
    Ok(Json(wire_state(&state)))
}

pub async fn start_development(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(_payload): Json<serde_json::Value>,
) -> Result<Json<RequirementDevelopmentWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_dev_err(&headers))?;
    let file_path = crate::requirement::requirement_file_path(&workspace, &id);
    if !file_path.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }
    let detail = load_requirement_detail(&workspace, &id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !matches!(detail.phase, RequirementPhase::Development) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "requirement_phase_invalid"),
        ));
    }
    let feature_branch = format!("feat-{}", id);
    let state_path = dev_state_path(&workspace, &id);
    let state = if state_path.exists() {
        let mut s = load_dev_state(&workspace, &id).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;
        s.feature_branch = feature_branch.clone();
        s.feature_branch_created = true;
        s
    } else {
        DevStateFile {
            requirement_id: id.clone(),
            feature_branch: feature_branch.clone(),
            feature_branch_created: true,
            tasks: vec![],
            current_task_id: None,
        }
    };
    save_dev_state(&workspace, &id, &state).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    if let Some(main_repo) = workspace.join("repos").join("main").join("main")
        .exists()
        .then(|| workspace.join("repos").join("main").join("main"))
    {
        let result = std::process::Command::new("git")
            .current_dir(&main_repo)
            .args(["checkout", "-b", &feature_branch])
            .output();
        let _ = result;
    }
    Ok(Json(wire_state(&state)))
}

pub async fn create_task(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<Json<RequirementDevelopmentWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_dev_err(&headers))?;
    let title = payload.title.trim();
    if title.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "task_title_empty"),
        ));
    }
    let mut state = load_dev_state(&workspace, &id).map_err(|e| {
        if e == "development_not_started" {
            (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(&headers, "development_not_started"),
            )
        } else {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        }
    })?;
    if !state.feature_branch_created {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "feature_branch_not_created"),
        ));
    }
    let task_num = state.tasks.len() + 1;
    let task_id = format!("task-{task_num:03}");
    let branch = format!("{}-{}", state.feature_branch, task_id);
    let now = now_ms();
    let task = DevTask {
        id: task_id.clone(),
        title: title.to_string(),
        assignee: payload.assignee.filter(|s| !s.trim().is_empty()),
        branch: branch.clone(),
        status: DevTaskStatus::BranchCreated,
        progress: 0,
        created_at_ms: now,
        updated_at_ms: now,
    };
    state.tasks.push(task);
    state.current_task_id = Some(task_id.clone());
    save_dev_state(&workspace, &id, &state).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    save_task_content(
        &workspace,
        &id,
        &task_id,
        &format!("# {title}\n\nBranch: `{branch}`\n"),
    )
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Some(main_repo) = workspace.join("repos").join("main").join("main")
        .exists()
        .then(|| workspace.join("repos").join("main").join("main"))
    {
        let result = std::process::Command::new("git")
            .current_dir(&main_repo)
            .args(["checkout", "-b", &branch])
            .output();
        let _ = result;
    }
    Ok(Json(wire_state(&state)))
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTaskRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub progress: Option<u8>,
}

pub async fn update_task(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath((id, task_id)): AxumPath<(String, String)>,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<Json<RequirementDevelopmentWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_dev_err(&headers))?;
    let mut state = load_dev_state(&workspace, &id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    let task = state.tasks.iter_mut().find(|t| t.id == task_id).ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "task_not_found"),
        )
    })?;
    if let Some(title) = payload.title {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "task_title_empty"),
            ));
        }
        task.title = trimmed.to_string();
    }
    task.assignee = payload.assignee.filter(|s| !s.trim().is_empty());
    if let Some(progress) = payload.progress {
        task.progress = progress.min(100);
    }
    task.updated_at_ms = now_ms();
    save_dev_state(&workspace, &id, &state).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    Ok(Json(wire_state(&state)))
}

pub async fn delete_task(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath((id, task_id)): AxumPath<(String, String)>,
) -> Result<Json<RequirementDevelopmentWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_dev_err(&headers))?;
    let mut state = load_dev_state(&workspace, &id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    let idx = state.tasks.iter().position(|t| t.id == task_id).ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "task_not_found"),
        )
    })?;
    state.tasks.remove(idx);
    if state.current_task_id.as_deref() == Some(&task_id) {
        state.current_task_id = state.tasks.last().map(|t| t.id.clone());
    }
    let _ = fs::remove_file(task_file_path(&workspace, &id, &task_id));
    save_dev_state(&workspace, &id, &state).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    Ok(Json(wire_state(&state)))
}

#[derive(Debug, Clone, Deserialize)]
pub struct DevTaskActionPayload {
    pub action: String,
}

pub async fn task_action(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath((id, task_id)): AxumPath<(String, String)>,
    Json(payload): Json<DevTaskActionPayload>,
) -> Result<Json<RequirementDevelopmentWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_dev_err(&headers))?;
    let mut state = load_dev_state(&workspace, &id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    let task = state.tasks.iter_mut().find(|t| t.id == task_id).ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "task_not_found"),
        )
    })?;
    match payload.action.as_str() {
        "start_development" => {
            if !matches!(task.status, DevTaskStatus::BranchCreated) {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            task.status = DevTaskStatus::InDevelopment;
        }
        "complete_development" => {
            if !matches!(task.status, DevTaskStatus::InDevelopment) {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            task.status = DevTaskStatus::DevComplete;
            task.progress = 100;
        }
        "start_review" => {
            if !matches!(task.status, DevTaskStatus::DevComplete) {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            task.status = DevTaskStatus::InReview;
        }
        "complete_review" => {
            if !matches!(task.status, DevTaskStatus::InReview) {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            task.status = DevTaskStatus::ReviewComplete;
        }
        "merge" => {
            if !matches!(task.status, DevTaskStatus::ReviewComplete) {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            task.status = DevTaskStatus::Merged;
            if let Some(main_repo) = workspace.join("repos").join("main").join("main")
                .exists()
                .then(|| workspace.join("repos").join("main").join("main"))
            {
                let _ = std::process::Command::new("git")
                    .current_dir(&main_repo)
                    .args(["checkout", &state.feature_branch])
                    .output();
                let _ = std::process::Command::new("git")
                    .current_dir(&main_repo)
                    .args(["merge", "--no-ff", &task.branch])
                    .output();
            }
        }
        _ => {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "task_action_unknown"),
            ));
        }
    }
    task.updated_at_ms = now_ms();
    state.current_task_id = Some(task_id);
    save_dev_state(&workspace, &id, &state).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    Ok(Json(wire_state(&state)))
}

fn map_dev_err(
    headers: &HeaderMap,
) -> impl Fn(anyhow::Error) -> (axum::http::StatusCode, String) + '_ {
    move |err| {
        let key = err.to_string();
        let known = [
            "task_title_empty",
            "task_not_found",
            "task_action_invalid",
            "task_action_unknown",
            "feature_branch_not_created",
            "development_not_started",
        ];
        if known.contains(&key.as_str()) {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(headers, &key),
            );
        }
        (
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(headers, "task_not_found"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-dev-test-{unique}"))
    }

    #[test]
    fn dev_task_status_next_follows_pipeline() {
        assert_eq!(
            DevTaskStatus::BranchCreated.next_status(),
            Some(DevTaskStatus::InDevelopment)
        );
        assert_eq!(
            DevTaskStatus::InDevelopment.next_status(),
            Some(DevTaskStatus::DevComplete)
        );
        assert_eq!(DevTaskStatus::Merged.next_status(), None);
    }

    #[test]
    fn dev_task_status_as_str_matches_serde_names() {
        assert_eq!(DevTaskStatus::InReview.as_str(), "in_review");
    }

    #[test]
    fn dev_state_save_and_load_roundtrip() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let state = DevStateFile {
            requirement_id: "feat-a".into(),
            feature_branch: "feat-feat-a".into(),
            feature_branch_created: true,
            tasks: vec![DevTask {
                id: "task-001".into(),
                title: "Implement API".into(),
                assignee: Some("alice".into()),
                branch: "feat-feat-a-task-001".into(),
                status: DevTaskStatus::BranchCreated,
                progress: 0,
                created_at_ms: 1,
                updated_at_ms: 2,
            }],
            current_task_id: Some("task-001".into()),
        };
        save_dev_state(&workspace, "feat-a", &state).unwrap();
        let loaded = load_dev_state(&workspace, "feat-a").unwrap();
        assert_eq!(loaded.requirement_id, "feat-a");
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].title, "Implement API");
        let wire = wire_state(&loaded);
        assert_eq!(wire.tasks[0].status, "branch_created");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn save_task_content_writes_markdown_file() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        save_task_content(&workspace, "feat-a", "task-001", "# Title\n").unwrap();
        let path = task_file_path(&workspace, "feat-a", "task-001");
        assert!(path.exists());
        let raw = fs::read_to_string(path).unwrap();
        assert!(raw.contains("# Title"));
        let _ = fs::remove_dir_all(&workspace);
    }
}
