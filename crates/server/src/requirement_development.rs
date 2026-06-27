use crate::{
    autonomy_trigger::mark_employee_for_autonomy,
    dev_task_executor,
    employee::list_employee_records,
    i18n,
    requirement::{
        load_requirement_detail, normalize_requirement_id,
        requirement_dir,
    },
    requirement_agents::{
        pick_employee_for_role, spawn_requirement_agent_task, AgentDispatchWire, AgentTaskSpec,
    },
    tasks::TaskKind,
    tools::driver::ToolChatMessage,
    work_task::{
        create_work_task, delete_work_task, dev_status, filter_work_tasks,
        is_development_task, list_work_tasks, load_work_task, save_work_task,
        set_dev_status, task_branch, update_work_task, work_tasks_root, BIZ_TYPE_REQUIREMENT,
        CreateWorkTaskParams, TASK_KIND_DEVELOPMENT, WorkTask, WorkTaskFilter, WorkTaskStatus,
    },
    AppState,
};
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    Json,
};
use crate::work_task::now_ms as work_task_now_ms;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::Path,
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

    pub fn from_str(raw: &str) -> Option<Self> {
        match raw {
            "branch_created" => Some(Self::BranchCreated),
            "in_development" => Some(Self::InDevelopment),
            "dev_complete" => Some(Self::DevComplete),
            "in_review" => Some(Self::InReview),
            "review_complete" => Some(Self::ReviewComplete),
            "merged" => Some(Self::Merged),
            _ => None,
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

    fn to_work_status(&self) -> WorkTaskStatus {
        match self {
            Self::Merged => WorkTaskStatus::Completed,
            Self::BranchCreated => WorkTaskStatus::Pending,
            _ => WorkTaskStatus::InProgress,
        }
    }

    fn from_planned_status(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "pending" | "branch_created" => Self::BranchCreated,
            "in_progress" | "in_development" | "running" => Self::InDevelopment,
            "dev_complete" | "complete" | "completed" => Self::DevComplete,
            "in_review" | "review" => Self::InReview,
            "review_complete" => Self::ReviewComplete,
            "merged" => Self::Merged,
            _ => Self::BranchCreated,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyDevTask {
    pub id: String,
    pub title: String,
    #[serde(default)]
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
    tasks: Vec<LegacyDevTask>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone_phase: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DevStateFileLoose {
    #[serde(default)]
    requirement_id: Option<String>,
    #[serde(default)]
    feature_branch: Option<String>,
    #[serde(default)]
    feature_branch_created: Option<bool>,
    #[serde(default)]
    current_task_id: Option<String>,
    #[serde(default)]
    milestone: Option<String>,
    #[serde(default)]
    milestone_phase: Option<String>,
    #[serde(default)]
    tasks: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlannedDevTask {
    pub title: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub milestone: Option<String>,
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
    pub biz_type: String,
    pub biz_id: String,
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
    let loose: DevStateFileLoose = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let mut state = DevStateFile {
        requirement_id: loose
            .requirement_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| id.to_string()),
        feature_branch: loose
            .feature_branch
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("feat-{id}")),
        feature_branch_created: loose.feature_branch_created.unwrap_or(false),
        tasks: Vec::new(),
        current_task_id: loose.current_task_id,
        milestone: loose.milestone,
        milestone_phase: loose.milestone_phase,
    };
    if let Some(tasks) = loose.tasks {
        migrate_tasks_value(workspace, id, &mut state, &tasks)?;
    }
    migrate_legacy_dev_tasks(workspace, id, &mut state)?;
    Ok(state)
}

fn migrate_tasks_value(
    workspace: &Path,
    requirement_id: &str,
    state: &mut DevStateFile,
    tasks: &Value,
) -> Result<(), String> {
    match tasks {
        Value::Array(items) => {
            for item in items {
                let legacy: LegacyDevTask =
                    serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
                state.tasks.push(legacy);
            }
        }
        Value::Object(map) if !map.is_empty() => {
            fs::create_dir_all(work_tasks_root(workspace)).map_err(|e| e.to_string())?;
            let ts = work_task_now_ms();
            let default_milestone = state.milestone.clone();
            for (task_id, item) in map {
                let planned: PlannedDevTask =
                    serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
                let dev_status = DevTaskStatus::from_planned_status(
                    planned.status.as_deref().unwrap_or("pending"),
                );
                let branch = format!("{}-{}", state.feature_branch, task_id);
                let description = fs::read_to_string(task_file_path(workspace, requirement_id, task_id))
                    .unwrap_or_default();
                let mut metadata = serde_json::json!({
                    "task_kind": TASK_KIND_DEVELOPMENT,
                    "branch": branch,
                    "dev_status": dev_status.as_str(),
                });
                if let Some(milestone) = planned
                    .milestone
                    .clone()
                    .or_else(|| default_milestone.clone())
                {
                    metadata["milestone"] = Value::String(milestone);
                }
                if let Some(existing) = load_work_task(workspace, task_id).ok() {
                    if is_development_task(&existing) && existing.biz_id == requirement_id {
                        continue;
                    }
                }
                let task = WorkTask {
                    id: task_id.clone(),
                    biz_type: BIZ_TYPE_REQUIREMENT.to_string(),
                    biz_id: requirement_id.to_string(),
                    title: planned.title,
                    description,
                    assignee: None,
                    status: dev_status.to_work_status(),
                    progress: 0,
                    auto_executable: true,
                    metadata,
                    created_at_ms: ts,
                    updated_at_ms: ts,
                    agent_task_id: None,
                };
                save_work_task(workspace, &task)?;
            }
            save_dev_state(workspace, requirement_id, state)?;
        }
        _ => {}
    }
    Ok(())
}

fn save_dev_state(workspace: &Path, id: &str, state: &DevStateFile) -> Result<(), String> {
    let path = dev_state_path(workspace, id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, serde_json::to_string_pretty(state).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn migrate_legacy_dev_tasks(
    workspace: &Path,
    requirement_id: &str,
    state: &mut DevStateFile,
) -> Result<(), String> {
    if state.tasks.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(work_tasks_root(workspace)).map_err(|e| e.to_string())?;
    for legacy in state.tasks.drain(..) {
        let status = legacy.status.to_work_status();
        let task = WorkTask {
            id: legacy.id.clone(),
            biz_type: BIZ_TYPE_REQUIREMENT.to_string(),
            biz_id: requirement_id.to_string(),
            title: legacy.title.clone(),
            description: String::new(),
            assignee: legacy.assignee.clone(),
            status,
            progress: legacy.progress,
            auto_executable: true,
            metadata: serde_json::json!({
                "task_kind": TASK_KIND_DEVELOPMENT,
                "branch": legacy.branch,
                "dev_status": legacy.status.as_str(),
            }),
            created_at_ms: legacy.created_at_ms,
            updated_at_ms: legacy.updated_at_ms,
            agent_task_id: None,
        };
        save_work_task(workspace, &task)?;
        if let Ok(content) = fs::read_to_string(task_file_path(workspace, requirement_id, &legacy.id))
        {
            if !content.trim().is_empty() {
                let mut migrated = task.clone();
                migrated.description = content;
                save_work_task(workspace, &migrated)?;
            }
        }
    }
    save_dev_state(workspace, requirement_id, state)?;
    Ok(())
}

pub fn reconcile_development_work_tasks(
    workspace: &Path,
    requirement_id: &str,
) -> Result<(), String> {
    let id = normalize_requirement_id(requirement_id).map_err(|e| e.to_string())?;
    if dev_state_path(workspace, &id).exists() {
        let mut state = load_dev_state(workspace, &id)?;
        migrate_legacy_dev_tasks(workspace, &id, &mut state)?;
    }

    let tasks_dir = dev_tasks_dir(workspace, &id);
    if !tasks_dir.exists() {
        return Ok(());
    }

    let state = try_load_dev_state(workspace, &id);
    let feature_branch = state
        .as_ref()
        .map(|value| value.feature_branch.clone())
        .unwrap_or_else(|| format!("feat-{id}"));
    let existing = list_development_work_tasks(workspace, &id)?;
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
        let description = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let title = description
            .lines()
            .find(|line| line.starts_with("# "))
            .map(|line| line.trim_start_matches("# ").trim())
            .filter(|line| !line.is_empty())
            .unwrap_or(&task_id)
            .to_string();
        let branch = format!("{feature_branch}-{task_id}");

        if let Some(task) = existing.iter().find(|task| task.id == task_id) {
            let assignee_missing = task.assignee.as_deref().unwrap_or("").trim().is_empty();
            let description_missing = task.description.trim().is_empty();
            if assignee_missing || description_missing {
                update_work_task(workspace, &task_id, |task| {
                    if assignee_missing {
                        if let Some(ref assignee) = default_assignee {
                            task.assignee = Some(assignee.clone());
                        }
                    }
                    if description_missing && !description.trim().is_empty() {
                        task.description = description.clone();
                    }
                    Ok(())
                })?;
            }
            continue;
        }

        create_work_task(
            workspace,
            CreateWorkTaskParams {
                id: Some(&task_id),
                biz_type: BIZ_TYPE_REQUIREMENT,
                biz_id: &id,
                title: &title,
                description: &description,
                assignee: default_assignee.as_deref(),
                auto_executable: true,
                metadata: serde_json::json!({
                    "task_kind": TASK_KIND_DEVELOPMENT,
                    "branch": branch,
                    "dev_status": DevTaskStatus::BranchCreated.as_str(),
                }),
            },
        )?;
    }
    Ok(())
}

fn list_development_work_tasks(
    workspace: &Path,
    requirement_id: &str,
) -> Result<Vec<WorkTask>, String> {
    Ok(filter_work_tasks(
        list_work_tasks(workspace)?,
        &WorkTaskFilter {
            biz_type: Some(BIZ_TYPE_REQUIREMENT.to_string()),
            biz_id: Some(requirement_id.to_string()),
            task_kind: Some(TASK_KIND_DEVELOPMENT.to_string()),
            ..Default::default()
        },
    ))
}

fn dev_status_of(task: &WorkTask) -> DevTaskStatus {
    dev_status(task)
        .and_then(DevTaskStatus::from_str)
        .unwrap_or(DevTaskStatus::BranchCreated)
}

/// Returns development work tasks that are still pending (`branch_created`) and can
/// be picked up by `employee_id` during exploration.
///
/// Ordering reflects the exploration priority: tasks already assigned to the
/// employee come first (oldest first), followed by unassigned tasks (oldest
/// first). Tasks assigned to a different employee, non-development tasks, tasks
/// that are not auto-executable, and tasks that have already moved past
/// `branch_created` are excluded.
pub fn list_claimable_dev_tasks(workspace: &Path, employee_id: &str) -> Vec<WorkTask> {
    let mut assigned: Vec<WorkTask> = Vec::new();
    let mut unassigned: Vec<WorkTask> = Vec::new();
    for task in list_work_tasks(workspace).unwrap_or_default() {
        if !is_development_task(&task) || !task.auto_executable {
            continue;
        }
        if task.status != WorkTaskStatus::Pending {
            continue;
        }
        if dev_status_of(&task) != DevTaskStatus::BranchCreated {
            continue;
        }
        match task
            .assignee
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(owner) if owner == employee_id => assigned.push(task),
            Some(_) => continue,
            None => unassigned.push(task),
        }
    }
    assigned.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms));
    unassigned.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms));
    assigned.into_iter().chain(unassigned).collect()
}

/// Claims a pending development work task for `employee_id` and transitions it into
/// the `in_development` state so the development agent can begin execution.
///
/// Mirrors the `start_development` branch of [`task_action`]: assigns the employee
/// when the task is unassigned, advances the dev/work status, and records the task
/// as the requirement's current task. Returns the requirement id so callers can run
/// the development agent against the correct branch and repository.
pub fn claim_dev_task(
    workspace: &Path,
    task_id: &str,
    employee_id: &str,
) -> Result<String, String> {
    let task = load_work_task(workspace, task_id)?;
    if !is_development_task(&task) {
        return Err("task_not_found".to_string());
    }
    if dev_status_of(&task) != DevTaskStatus::BranchCreated {
        return Err("task_action_invalid".to_string());
    }
    let requirement_id = task.biz_id.clone();
    update_work_task(workspace, task_id, |task| {
        if task
            .assignee
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            task.assignee = Some(employee_id.to_string());
        }
        set_dev_status(task, DevTaskStatus::InDevelopment.as_str());
        task.status = DevTaskStatus::InDevelopment.to_work_status();
        Ok(())
    })?;
    if let Ok(mut state) = load_dev_state(workspace, &requirement_id) {
        state.current_task_id = Some(task_id.to_string());
        let _ = save_dev_state(workspace, &requirement_id, &state);
    }
    Ok(requirement_id)
}

fn wire_task(task: &WorkTask) -> DevTaskWire {
    DevTaskWire {
        id: task.id.clone(),
        title: task.title.clone(),
        assignee: task.assignee.clone(),
        branch: task_branch(task).unwrap_or("").to_string(),
        status: dev_status_of(task).as_str().to_string(),
        progress: task.progress,
        created_at_ms: task.created_at_ms,
        updated_at_ms: task.updated_at_ms,
        biz_type: task.biz_type.clone(),
        biz_id: task.biz_id.clone(),
    }
}

fn wire_state(workspace: &Path, state: &DevStateFile) -> Result<RequirementDevelopmentWire, String> {
    let tasks = list_development_work_tasks(workspace, &state.requirement_id)?;
    Ok(RequirementDevelopmentWire {
        requirement_id: state.requirement_id.clone(),
        feature_branch: state.feature_branch.clone(),
        feature_branch_created: state.feature_branch_created,
        tasks: tasks.iter().map(wire_task).collect(),
        current_task_id: state.current_task_id.clone(),
    })
}

pub fn try_load_dev_state(workspace: &Path, id: &str) -> Option<DevStateFile> {
    load_dev_state(workspace, id).ok()
}

fn create_feature_branch(workspace: &Path, feature_branch: &str) {
    if let Some(main_repo) = workspace
        .join("repos")
        .join("main")
        .exists()
        .then(|| workspace.join("repos").join("main"))
    {
        let _ = std::process::Command::new("git")
            .current_dir(&main_repo)
            .args(["checkout", "-b", feature_branch])
            .output();
    }
}

fn create_task_branch(workspace: &Path, branch: &str) {
    if let Some(main_repo) = workspace
        .join("repos")
        .join("main")
        .exists()
        .then(|| workspace.join("repos").join("main"))
    {
        let _ = std::process::Command::new("git")
            .current_dir(&main_repo)
            .args(["checkout", "-b", branch])
            .output();
    }
}

pub fn ensure_development_started(
    workspace: &Path,
    requirement_id: &str,
) -> Result<DevStateFile, String> {
    let id = normalize_requirement_id(requirement_id).map_err(|e| e.to_string())?;
    let file_path = crate::requirement::requirement_file_path(workspace, &id);
    if !file_path.exists() {
        return Err("requirement_not_found".to_string());
    }
    let _detail = load_requirement_detail(workspace, &id).map_err(|e| e.to_string())?;
    let feature_branch = format!("feat-{id}");
    let state_path = dev_state_path(workspace, &id);
    let state = if state_path.exists() {
        let mut state = load_dev_state(workspace, &id)?;
        state.feature_branch = feature_branch.clone();
        state.feature_branch_created = true;
        state
    } else {
        DevStateFile {
            requirement_id: id.clone(),
            feature_branch: feature_branch.clone(),
            feature_branch_created: true,
            tasks: vec![],
            current_task_id: None,
            milestone: None,
            milestone_phase: None,
        }
    };
    save_dev_state(workspace, &id, &state)?;
    create_feature_branch(workspace, &feature_branch);
    Ok(state)
}

pub fn add_development_task(
    workspace: &Path,
    requirement_id: &str,
    title: &str,
    description: &str,
    assignee: Option<&str>,
) -> Result<WorkTask, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("task_title_empty".to_string());
    }
    let id = normalize_requirement_id(requirement_id).map_err(|e| e.to_string())?;
    let state = ensure_development_started(workspace, &id)?;
    let existing = list_development_work_tasks(workspace, &id)?;
    if let Some(found) = existing.iter().find(|task| task.title == title) {
        return Ok(found.clone());
    }
    let task_num = existing.len() + 1;
    let task_id = format!("task-{task_num:03}");
    let branch = format!("{}-{}", state.feature_branch, task_id);
    create_work_task(
        workspace,
        CreateWorkTaskParams {
            id: Some(&task_id),
            biz_type: BIZ_TYPE_REQUIREMENT,
            biz_id: &id,
            title,
            description,
            assignee,
            auto_executable: true,
            metadata: serde_json::json!({
                "task_kind": TASK_KIND_DEVELOPMENT,
                "branch": branch,
                "dev_status": DevTaskStatus::BranchCreated.as_str(),
            }),
        },
    )?;
    let mut state = load_dev_state(workspace, &id)?;
    state.current_task_id = Some(task_id.clone());
    save_dev_state(workspace, &id, &state)?;
    save_task_content(
        workspace,
        &id,
        &task_id,
        &format!("# {title}\n\n{description}\n\nBranch: `{branch}`\n"),
    )?;
    create_task_branch(workspace, &branch);
    if let Some(assignee) = assignee.filter(|value| !value.trim().is_empty()) {
        let _ = mark_employee_for_autonomy(workspace, assignee);
    }
    load_work_task(workspace, &task_id)
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
    wire_state(&workspace, &state).map(Json).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })
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
    let _detail = load_requirement_detail(&workspace, &id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let state = ensure_development_started(&workspace, &id).map_err(|e| {
        if e == "requirement_not_found" {
            (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(&headers, "requirement_not_found"),
            )
        } else {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        }
    })?;
    if list_development_work_tasks(&workspace, &id)
        .map(|tasks| tasks.is_empty())
        .unwrap_or(true)
    {
        if let Ok(employees) = list_employee_records(&workspace) {
            for employee in employees {
                let _ = mark_employee_for_autonomy(&workspace, &employee.id);
            }
        }
    }
    wire_state(&workspace, &state).map(Json).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })
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
    if !state.feature_branch_created {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "feature_branch_not_created"),
        ));
    }
    add_development_task(
        &workspace,
        &id,
        title,
        &format!("# {title}\n"),
        payload.assignee.as_deref(),
    )
    .map_err(|e| {
        if e == "task_title_empty" {
            (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "task_title_empty"),
            )
        } else {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        }
    })?;
    let state = load_dev_state(&workspace, &id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    wire_state(&workspace, &state).map(Json).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })
}

fn build_split_messages(
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
            existing.push_str(&format!(
                "- {} — {} [{}]\n",
                task.id,
                task.title,
                dev_status_of(task).as_str()
            ));
        }
    }
    let system = format!(
        r#"You are a senior engineer breaking a requirement into concrete development tasks.

## Working directory
This directory is the requirement `{requirement_id}` package. The requirement body is in `requirement.md`. Development tasks live as Markdown files under `development/tasks/`.

## Existing development tasks
{existing}

## Task
1. Read `requirement.md` and the existing development tasks above.
2. Decide which additional implementation tasks are still needed (do NOT duplicate existing tasks). Prefer 2-6 focused tasks that can each be implemented in a separate session.
3. For each new task, create a Markdown file `development/tasks/task-NNN.md` where NNN is a zero-padded 3-digit number. Start numbering at {next_index:03} and increment. Do not overwrite existing files.
4. Each task file MUST start with a level-1 heading containing the task title, for example `# Implement login API`, followed by a short description of the work and acceptance criteria.
5. Reply briefly listing the task files you created and their titles.

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
                "Split requirement **{title}** (`{requirement_id}`) into development tasks now.\n\n---\n\n{content}"
            ),
        },
    ]
}

/// Dispatches a code-agent task that splits the requirement into development
/// tasks. A suitable engineering employee is assigned; new task files written by
/// the agent are reconciled into work tasks once the run completes.
pub async fn split_development_tasks(
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
    // Ensure the feature branch + development state exist before planning tasks.
    ensure_development_started(&workspace, &id).map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    let Some(employee) = pick_employee_for_role(&workspace, "engineering") else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "requirement_no_employees"),
        ));
    };

    let existing = list_development_work_tasks(&workspace, &id).unwrap_or_default();
    let next_index = existing.len() + 1;
    let messages = build_split_messages(&id, &detail.title, &detail.content, &existing, next_index);
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workdir = requirement_dir(&workspace, &id);
    let reconcile_id = id.clone();
    let task = spawn_requirement_agent_task(
        &workspace,
        &tools,
        &employee.id,
        AgentTaskSpec {
            kind: TaskKind::RequirementAgent,
            content: format!("Split development tasks for `{id}`"),
            workdir,
            messages,
            context: serde_json::json!({ "requirement_id": id }),
        },
        move |ws| {
            let _ = reconcile_development_work_tasks(ws, &reconcile_id);
        },
    )
    .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(Json(AgentDispatchWire::from_employee_task(&employee, &task)))
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
    let _state = load_dev_state(&workspace, &id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    let title = payload.title.clone();
    let assignee = payload.assignee.clone();
    let progress = payload.progress;
    let _ = update_work_task(&workspace, &task_id, |task| {
        if !is_development_task(task) || task.biz_id != id {
            return Err("task_not_found".to_string());
        }
        if let Some(ref title) = title {
            let trimmed = title.trim();
            if trimmed.is_empty() {
                return Err("task_title_empty".to_string());
            }
            task.title = trimmed.to_string();
        }
        task.assignee = assignee.clone().filter(|s| !s.trim().is_empty());
        if let Some(progress) = progress {
            task.progress = progress.min(100);
        }
        Ok(())
    })
    .map_err(|e| {
        if e == "task_not_found" {
            (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(&headers, "task_not_found"),
            )
        } else if e == "task_title_empty" {
            (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "task_title_empty"),
            )
        } else {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        }
    })?;
    let state = load_dev_state(&workspace, &id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    wire_state(&workspace, &state).map(Json).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })
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
    let task = load_work_task(&workspace, &task_id).map_err(|_| {
        (
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "task_not_found"),
        )
    })?;
    if !is_development_task(&task) || task.biz_id != id {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "task_not_found"),
        ));
    }
    delete_work_task(&workspace, &task_id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    if state.current_task_id.as_deref() == Some(&task_id) {
        let remaining = list_development_work_tasks(&workspace, &id).unwrap_or_default();
        state.current_task_id = remaining.last().map(|t| t.id.clone());
        save_dev_state(&workspace, &id, &state).map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;
    }
    let _ = fs::remove_file(task_file_path(&workspace, &id, &task_id));
    wire_state(&workspace, &state).map(Json).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })
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
    let mut dev_state = load_dev_state(&workspace, &id).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    let task = load_work_task(&workspace, &task_id).map_err(|_| {
        (
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "task_not_found"),
        )
    })?;
    if !is_development_task(&task) || task.biz_id != id {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "task_not_found"),
        ));
    }
    let current = dev_status_of(&task);
    let employee_id = task.assignee.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "system".to_string());

    // Agent-driven actions that do not transition the dev status: continue the
    // implementation, or run a code review. These re-invoke the code agent off
    // the request path and return the current state immediately.
    if payload.action == "continue_development" || payload.action == "review_code" {
        let is_review = payload.action == "review_code";
        let valid = if is_review {
            current == DevTaskStatus::DevComplete || current == DevTaskStatus::InReview
        } else {
            current == DevTaskStatus::InDevelopment
        };
        if !valid {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "task_action_invalid"),
            ));
        }
        let tools = state.tools.read().expect("tools lock poisoned").clone();
        let workspace_path = workspace.clone();
        let run_task_id = task_id.clone();
        let run_requirement_id = id.clone();
        let run_employee_id = employee_id.clone();
        tokio::spawn(async move {
            let result = if is_review {
                dev_task_executor::execute_dev_task_review_streaming(
                    &workspace_path,
                    &tools,
                    &run_task_id,
                    &run_requirement_id,
                    &run_employee_id,
                )
                .await
            } else {
                dev_task_executor::execute_dev_task_streaming(
                    &workspace_path,
                    &tools,
                    &run_task_id,
                    &run_requirement_id,
                    &run_employee_id,
                )
                .await
            };
            if let Err(err) = result {
                tracing::warn!(error = %err, "dev task agent action failed");
            }
        });
        dev_state.current_task_id = Some(task_id);
        save_dev_state(&workspace, &id, &dev_state).map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        return wire_state(&workspace, &dev_state).map(Json).map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        });
    }

    let next = match payload.action.as_str() {
        "start_development" => {
            if current != DevTaskStatus::BranchCreated {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            DevTaskStatus::InDevelopment
        }
        "complete_development" => {
            if current != DevTaskStatus::InDevelopment {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            DevTaskStatus::DevComplete
        }
        "start_review" => {
            if current != DevTaskStatus::DevComplete {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            DevTaskStatus::InReview
        }
        "complete_review" => {
            if current != DevTaskStatus::InReview {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            DevTaskStatus::ReviewComplete
        }
        "merge" => {
            if current != DevTaskStatus::ReviewComplete {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    i18n::msg(&headers, "task_action_invalid"),
                ));
            }
            if let Some(main_repo) = workspace.join("repos").join("main")
                .exists()
                .then(|| workspace.join("repos").join("main"))
            {
                let branch = task_branch(&task).unwrap_or("").to_string();
                let _ = std::process::Command::new("git")
                    .current_dir(&main_repo)
                    .args(["checkout", &dev_state.feature_branch])
                    .output();
                let _ = std::process::Command::new("git")
                    .current_dir(&main_repo)
                    .args(["merge", "--no-ff", &branch])
                    .output();
            }
            DevTaskStatus::Merged
        }
        _ => {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "task_action_unknown"),
            ));
        }
    };
    let progress = if next == DevTaskStatus::DevComplete || next == DevTaskStatus::Merged {
        100
    } else {
        task.progress
    };
    let _ = update_work_task(&workspace, &task_id, |task| {
        set_dev_status(task, next.as_str());
        task.status = next.to_work_status();
        task.progress = progress;
        Ok(())
    });

    // When task enters InDevelopment, trigger code agent execution with git repo as working directory.
    // The agent run is long-running (it spawns and waits on an external coding CLI), so it MUST run
    // off the request path. Running it inline here previously blocked the Tokio worker thread and held
    // the `tools` read lock for the whole duration, which starved every other HTTP request (e.g.
    // `/api/workspace`, `/api/employees`) until the app was restarted. Clone the tools to release the
    // lock immediately and execute on the blocking thread pool, returning the response right away.
    if next == DevTaskStatus::InDevelopment {
        let tools = state.tools.read().expect("tools lock poisoned").clone();
        let workspace_path = workspace.clone();
        let run_task_id = task_id.clone();
        let run_requirement_id = id.clone();
        let run_employee_id = employee_id.clone();
        tokio::spawn(async move {
            if let Err(err) = dev_task_executor::execute_dev_task_streaming(
                &workspace_path,
                &tools,
                &run_task_id,
                &run_requirement_id,
                &run_employee_id,
            )
            .await
            {
                tracing::warn!(error = %err, "dev task streaming execution failed");
            }
        });
    }

    dev_state.current_task_id = Some(task_id);
    save_dev_state(&workspace, &id, &dev_state).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    wire_state(&workspace, &dev_state).map(Json).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })
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
    use crate::requirement::RequirementPhase;
    use crate::work_task::load_work_task;
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
    fn planned_dev_tasks_map_migrates_to_work_tasks() {
        use crate::requirement::{format_requirement_md, RequirementMeta, REQUIREMENT_FILE};

        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let req_dir = workspace.join("requirements").join("game");
        fs::create_dir_all(&req_dir).unwrap();
        fs::write(
            req_dir.join(REQUIREMENT_FILE),
            format_requirement_md(
                &RequirementMeta {
                    id: "game".into(),
                    title: "Game".into(),
                    phase: RequirementPhase::Development,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                },
                "## Scope",
            ),
        )
        .unwrap();
        fs::create_dir_all(req_dir.join("development")).unwrap();
        fs::write(
            dev_state_path(&workspace, "game"),
            r#"{
  "requirement_id": "game",
  "feature_branch": "feat-game",
  "feature_branch_created": true,
  "milestone": "M0",
  "tasks": {
    "task-001": { "milestone": "M0", "status": "pending", "title": "Bootstrap project" }
  }
}"#,
        )
        .unwrap();

        let loaded = load_dev_state(&workspace, "game").unwrap();
        assert_eq!(loaded.milestone.as_deref(), Some("M0"));
        let task = load_work_task(&workspace, "task-001").unwrap();
        assert_eq!(task.title, "Bootstrap project");
        assert_eq!(task.metadata["milestone"], "M0");
        let raw = fs::read_to_string(dev_state_path(&workspace, "game")).unwrap();
        assert!(!raw.contains("\"tasks\": {"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn legacy_dev_tasks_migrate_to_work_tasks() {
        use crate::requirement::{format_requirement_md, RequirementMeta, REQUIREMENT_FILE};

        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let req_dir = workspace.join("requirements").join("feat-a");
        fs::create_dir_all(&req_dir.join("development").join("tasks")).unwrap();
        fs::write(
            req_dir.join(REQUIREMENT_FILE),
            format_requirement_md(
                &RequirementMeta {
                    id: "feat-a".into(),
                    title: "Feature A".into(),
                    phase: RequirementPhase::Development,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                },
                "## Scope",
            ),
        )
        .unwrap();
        let state = DevStateFile {
            requirement_id: "feat-a".into(),
            feature_branch: "feat-feat-a".into(),
            feature_branch_created: true,
            tasks: vec![LegacyDevTask {
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
            milestone: None,
            milestone_phase: None,
        };
        save_dev_state(&workspace, "feat-a", &state).unwrap();
        save_task_content(&workspace, "feat-a", "task-001", "# Title\n").unwrap();

        let loaded = load_dev_state(&workspace, "feat-a").unwrap();
        assert!(loaded.tasks.is_empty());
        let tasks = list_development_work_tasks(&workspace, "feat-a").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Implement API");
        assert_eq!(tasks[0].biz_type, BIZ_TYPE_REQUIREMENT);
        assert_eq!(tasks[0].biz_id, "feat-a");
        assert_eq!(tasks[0].assignee.as_deref(), Some("alice"));
        let wire = wire_state(&workspace, &loaded).unwrap();
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

    fn make_dev_work_task(
        workspace: &Path,
        id: &str,
        requirement_id: &str,
        assignee: Option<&str>,
        dev_status: DevTaskStatus,
        work_status: WorkTaskStatus,
    ) {
        use crate::work_task::save_work_task;
        let ts = work_task_now_ms();
        let task = WorkTask {
            id: id.to_string(),
            biz_type: BIZ_TYPE_REQUIREMENT.to_string(),
            biz_id: requirement_id.to_string(),
            title: format!("Title {id}"),
            description: String::new(),
            assignee: assignee.map(str::to_string),
            status: work_status,
            progress: 0,
            auto_executable: true,
            metadata: serde_json::json!({
                "task_kind": TASK_KIND_DEVELOPMENT,
                "branch": format!("feat-{requirement_id}-{id}"),
                "dev_status": dev_status.as_str(),
            }),
            created_at_ms: ts,
            updated_at_ms: ts,
            agent_task_id: None,
        };
        save_work_task(workspace, &task).unwrap();
    }

    #[test]
    fn list_claimable_dev_tasks_prioritizes_assigned_then_unassigned() {
        let workspace = temp_workspace();
        fs::create_dir_all(work_tasks_root(&workspace)).unwrap();
        make_dev_work_task(
            &workspace,
            "task-unassigned",
            "auth",
            None,
            DevTaskStatus::BranchCreated,
            WorkTaskStatus::Pending,
        );
        make_dev_work_task(
            &workspace,
            "task-alice",
            "auth",
            Some("alice"),
            DevTaskStatus::BranchCreated,
            WorkTaskStatus::Pending,
        );
        make_dev_work_task(
            &workspace,
            "task-bob",
            "auth",
            Some("bob"),
            DevTaskStatus::BranchCreated,
            WorkTaskStatus::Pending,
        );
        make_dev_work_task(
            &workspace,
            "task-running",
            "auth",
            Some("alice"),
            DevTaskStatus::InDevelopment,
            WorkTaskStatus::InProgress,
        );

        let claimable = list_claimable_dev_tasks(&workspace, "alice");
        let ids: Vec<&str> = claimable.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["task-alice", "task-unassigned"]);

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn claim_dev_task_assigns_employee_and_starts_development() {
        let workspace = temp_workspace();
        fs::create_dir_all(work_tasks_root(&workspace)).unwrap();
        make_dev_work_task(
            &workspace,
            "task-001",
            "auth",
            None,
            DevTaskStatus::BranchCreated,
            WorkTaskStatus::Pending,
        );

        let requirement_id = claim_dev_task(&workspace, "task-001", "alice").unwrap();
        assert_eq!(requirement_id, "auth");

        let claimed = load_work_task(&workspace, "task-001").unwrap();
        assert_eq!(claimed.assignee.as_deref(), Some("alice"));
        assert_eq!(claimed.status, WorkTaskStatus::InProgress);
        assert_eq!(dev_status_of(&claimed), DevTaskStatus::InDevelopment);

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn claim_dev_task_rejects_non_pending_task() {
        let workspace = temp_workspace();
        fs::create_dir_all(work_tasks_root(&workspace)).unwrap();
        make_dev_work_task(
            &workspace,
            "task-001",
            "auth",
            Some("alice"),
            DevTaskStatus::InDevelopment,
            WorkTaskStatus::InProgress,
        );

        let err = claim_dev_task(&workspace, "task-001", "alice").unwrap_err();
        assert_eq!(err, "task_action_invalid");

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn ensure_development_started_persists_state_file() {
        use crate::requirement::{format_requirement_md, RequirementMeta, REQUIREMENT_FILE};

        let workspace = temp_workspace();
        let req_dir = workspace.join("requirements").join("auth");
        fs::create_dir_all(&req_dir).unwrap();
        fs::write(
            req_dir.join(REQUIREMENT_FILE),
            format_requirement_md(
                &RequirementMeta {
                    id: "auth".into(),
                    title: "User auth".into(),
                    phase: RequirementPhase::Development,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                },
                "## Scope\nImplement login.",
            ),
        )
        .unwrap();

        ensure_development_started(&workspace, "auth").unwrap();
        let path = dev_state_path(&workspace, "auth");
        assert!(path.exists());
        let loaded = load_dev_state(&workspace, "auth").unwrap();
        assert!(loaded.feature_branch_created);
        assert_eq!(loaded.feature_branch, "feat-auth");

        let _ = fs::remove_dir_all(&workspace);
    }
}
