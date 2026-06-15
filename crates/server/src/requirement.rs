use crate::{
    i18n,
    requirement_agents::{
        pick_employee_for_role, spawn_requirement_agent_task, AgentDispatchWire, AgentTaskSpec,
    },
    tasks::TaskKind,
    tools::driver::ToolChatMessage,
    work_task::{
        create_work_task, update_work_task, CreateWorkTaskParams, WorkTaskStatus,
        BIZ_TYPE_REQUIREMENT, TASK_KIND_OPTIMIZATION,
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
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const REQUIREMENT_FILE: &str = "requirement.md";

const PHASES: &[&str] = &[
    "collection",
    "development",
    "testing",
    "release",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequirementPhase {
    Collection,
    Development,
    Testing,
    Release,
}

impl RequirementPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Collection => "collection",
            Self::Development => "development",
            Self::Testing => "testing",
            Self::Release => "release",
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementMeta {
    pub id: String,
    pub title: String,
    pub phase: RequirementPhase,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementSummary {
    pub id: String,
    pub title: String,
    pub phase: RequirementPhase,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub dir_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementDetail {
    pub id: String,
    pub title: String,
    pub phase: RequirementPhase,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub dir_path: String,
    pub content: String,
    pub subdirs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRequirementRequest {
    pub title: String,
    pub phase: Option<RequirementPhase>,
    pub content: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRequirementRequest {
    pub title: Option<String>,
    pub phase: Option<RequirementPhase>,
    pub content: Option<String>,
}

fn workspace_root(state: &AppState) -> Option<PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

pub fn requirements_root(workspace: &Path) -> PathBuf {
    workspace.join("requirements")
}


pub fn ensure_requirements_root(workspace: &Path) -> anyhow::Result<PathBuf> {
    let root = requirements_root(workspace);
    fs::create_dir_all(&root)?;
    Ok(root)
}

pub fn phase_in_progress(phase: &RequirementPhase) -> bool {
    !matches!(phase, RequirementPhase::Release)
}

/// Lists all requirements under the workspace, newest updates first.
pub fn list_requirement_summaries(workspace: &Path) -> anyhow::Result<Vec<RequirementSummary>> {
    let root = requirements_root(workspace);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if !path.join(REQUIREMENT_FILE).exists() {
            continue;
        }
        if let Ok(item) = load_summary(workspace, &id) {
            items.push(item);
        }
    }
    items.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    Ok(items)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn normalize_requirement_id(raw: &str) -> anyhow::Result<String> {
    let trimmed = raw.trim().to_lowercase();
    if trimmed.is_empty() {
        anyhow::bail!("requirement_id_empty");
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        anyhow::bail!("requirement_id_invalid");
    }
    Ok(trimmed)
}

fn derive_requirement_id(title: &str) -> anyhow::Result<String> {
    let slug: String = title
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();
    let compact = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if compact.is_empty() {
        anyhow::bail!("requirement_id_invalid");
    }
    Ok(compact)
}

pub fn parse_requirement_md(raw: &str) -> anyhow::Result<(RequirementMeta, String)> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        anyhow::bail!("requirement_frontmatter_missing");
    }
    let rest = trimmed.strip_prefix("---").unwrap_or("").trim_start();
    let (yaml_part, body_part) = rest
        .split_once("---")
        .ok_or_else(|| anyhow::anyhow!("requirement_frontmatter_missing"))?;
    let meta: RequirementMeta = serde_yaml::from_str(yaml_part.trim())?;
    if !PHASES.contains(&meta.phase.as_str()) {
        anyhow::bail!("requirement_phase_invalid");
    }
    let content = body_part.trim_start().to_string();
    Ok((meta, content))
}

pub fn format_requirement_md(meta: &RequirementMeta, content: &str) -> String {
    let yaml = serde_yaml::to_string(meta).expect("requirement meta serializes");
    format!("---\n{yaml}---\n\n{content}")
}

pub fn requirement_dir(workspace: &Path, id: &str) -> PathBuf {
    requirements_root(workspace).join(id)
}

pub(crate) fn requirement_file_path(workspace: &Path, id: &str) -> PathBuf {
    requirement_dir(workspace, id).join(REQUIREMENT_FILE)
}

fn list_subdirs(dir: &Path) -> anyhow::Result<Vec<String>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    Ok(names)
}

pub(crate) fn load_requirement_detail(workspace: &Path, id: &str) -> anyhow::Result<RequirementDetail> {
    let dir = requirement_dir(workspace, id);
    let file_path = dir.join(REQUIREMENT_FILE);
    let raw = fs::read_to_string(&file_path)?;
    let (meta, content) = parse_requirement_md(&raw)?;
    let subdirs: Vec<String> = list_subdirs(&dir)?
        .into_iter()
        .filter(|name| name != REQUIREMENT_FILE)
        .collect();
    Ok(RequirementDetail {
        id: meta.id,
        title: meta.title,
        phase: meta.phase,
        created_at_ms: meta.created_at_ms,
        updated_at_ms: meta.updated_at_ms,
        dir_path: format!("requirements/{id}"),
        content,
        subdirs,
    })
}

fn load_summary(workspace: &Path, id: &str) -> anyhow::Result<RequirementSummary> {
    let detail = load_requirement_detail(workspace, id)?;
    Ok(RequirementSummary {
        id: detail.id,
        title: detail.title,
        phase: detail.phase,
        created_at_ms: detail.created_at_ms,
        updated_at_ms: detail.updated_at_ms,
        dir_path: detail.dir_path,
    })
}

pub async fn list_requirements(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Vec<RequirementSummary>>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let items = list_requirement_summaries(&workspace)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(items))
}

pub async fn get_requirement(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RequirementDetail>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_requirement_err(&headers))?;
    let file_path = requirement_file_path(&workspace, &id);
    if !file_path.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }
    load_requirement_detail(&workspace, &id)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn create_requirement(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<CreateRequirementRequest>,
) -> Result<Json<RequirementDetail>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let title = payload.title.trim();
    if title.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "requirement_title_empty"),
        ));
    }
    let id = if let Some(raw) = payload.id.as_deref() {
        normalize_requirement_id(raw).map_err(map_requirement_err(&headers))?
    } else {
        derive_requirement_id(title).map_err(map_requirement_err(&headers))?
    };
    let dir = requirement_dir(&workspace, &id);
    if dir.exists() {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "requirement_already_exists"),
        ));
    }
    fs::create_dir_all(&dir).map_err(internal_err)?;
    let now = now_ms();
    let phase = payload.phase.unwrap_or(RequirementPhase::Collection);
    let content = payload.content.unwrap_or_default();
    let meta = RequirementMeta {
        id: id.clone(),
        title: title.to_string(),
        phase,
        created_at_ms: now,
        updated_at_ms: now,
    };
    fs::write(
        dir.join(REQUIREMENT_FILE),
        format_requirement_md(&meta, &content),
    )
    .map_err(internal_err)?;
    load_requirement_detail(&workspace, &id)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

pub async fn update_requirement(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(payload): Json<UpdateRequirementRequest>,
) -> Result<Json<RequirementDetail>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_requirement_err(&headers))?;
    let file_path = requirement_file_path(&workspace, &id);
    if !file_path.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }
    let raw = fs::read_to_string(&file_path).map_err(internal_err)?;
    let (mut meta, mut content) =
        parse_requirement_md(&raw).map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                i18n::msg(&headers, "requirement_parse_failed"),
            )
        })?;
    if let Some(title) = payload.title {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "requirement_title_empty"),
            ));
        }
        meta.title = trimmed.to_string();
    }
    if let Some(phase) = payload.phase {
        meta.phase = phase;
    }
    if let Some(body) = payload.content {
        content = body;
    }
    meta.updated_at_ms = now_ms();
    fs::write(&file_path, format_requirement_md(&meta, &content)).map_err(internal_err)?;
    load_requirement_detail(&workspace, &id)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

fn internal_err(err: std::io::Error) -> (axum::http::StatusCode, String) {
    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

pub fn archived_requirement_root(workspace: &Path) -> PathBuf {
    workspace.join("requirements").join("archived")
}

/// Lists archived (abandoned) requirements from the archived directory.
pub fn list_archived_requirement_summaries(workspace: &Path) -> anyhow::Result<Vec<RequirementSummary>> {
    let root = archived_requirement_root(workspace);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if !path.join(REQUIREMENT_FILE).exists() {
            continue;
        }
        if let Ok(item) = load_archived_summary(workspace, &id) {
            items.push(item);
        }
    }
    items.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    Ok(items)
}

fn load_archived_summary(workspace: &Path, id: &str) -> anyhow::Result<RequirementSummary> {
    let dir = archived_requirement_root(workspace).join(id);
    let file_path = dir.join(REQUIREMENT_FILE);
    let raw = fs::read_to_string(&file_path)?;
    let (meta, _content) = parse_requirement_md(&raw)?;
    Ok(RequirementSummary {
        id: meta.id,
        title: meta.title,
        phase: meta.phase,
        created_at_ms: meta.created_at_ms,
        updated_at_ms: meta.updated_at_ms,
        dir_path: format!("requirements/archived/{id}"),
    })
}

pub async fn list_archived_requirements(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Vec<RequirementSummary>>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let items = list_archived_requirement_summaries(&workspace)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(items))
}

pub async fn abandon_requirement(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RequirementDetail>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_requirement_err(&headers))?;
    let file_path = requirement_file_path(&workspace, &id);
    if !file_path.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }
    let raw = fs::read_to_string(&file_path).map_err(internal_err)?;
    let (mut meta, content) =
        parse_requirement_md(&raw).map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                i18n::msg(&headers, "requirement_parse_failed"),
            )
        })?;
    meta.updated_at_ms = now_ms();
    fs::write(&file_path, format_requirement_md(&meta, &content)).map_err(internal_err)?;

    let archived_root = archived_requirement_root(&workspace);
    fs::create_dir_all(&archived_root)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let active_dir = requirement_dir(&workspace, &id);
    let archived_dir = archived_root.join(&id);
    if archived_dir.exists() {
        fs::remove_dir_all(&archived_dir)
            .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    }
    fs::rename(&active_dir, &archived_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let summary = load_archived_summary(&workspace, &id)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(RequirementDetail {
        id: summary.id,
        title: summary.title,
        phase: summary.phase,
        created_at_ms: summary.created_at_ms,
        updated_at_ms: summary.updated_at_ms,
        dir_path: summary.dir_path,
        content,
        subdirs: Vec::new(),
    }))
}

/// Reinstates an archived requirement: moves it back from the archived folder.
pub async fn reinstate_requirement(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RequirementDetail>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_requirement_err(&headers))?;
    let archived_root = archived_requirement_root(&workspace);
    let archived_dir = archived_root.join(&id);
    if !archived_dir.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_archived_not_found"),
        ));
    }

    let active_dir = requirement_dir(&workspace, &id);
    if active_dir.exists() {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "requirement_already_exists"),
        ));
    }
    fs::rename(&archived_dir, &active_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let file_path = active_dir.join(REQUIREMENT_FILE);
    let raw = fs::read_to_string(&file_path).map_err(internal_err)?;
    let (mut meta, content) = parse_requirement_md(&raw).map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            i18n::msg(&headers, "requirement_parse_failed"),
        )
    })?;
    meta.updated_at_ms = now_ms();
    fs::write(&file_path, format_requirement_md(&meta, &content)).map_err(internal_err)?;

    load_requirement_detail(&workspace, &id)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

/// Permanently deletes an archived requirement.
pub async fn hard_delete_requirement(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id).map_err(map_requirement_err(&headers))?;
    let archived_root = archived_requirement_root(&workspace);
    let archived_dir = archived_root.join(&id);
    if !archived_dir.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_archived_not_found"),
        ));
    }

    fs::remove_dir_all(&archived_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok((axum::http::StatusCode::NO_CONTENT, Json(serde_json::json!({}))))
}





fn build_optimize_messages(
    requirement_id: &str,
    title: &str,
    content: &str,
) -> Vec<ToolChatMessage> {
    let system = format!(
        r#"You are a product specialist optimizing a requirement during the collection phase.

## Working directory
This directory is the requirement `{requirement_id}` package. The requirement body is in `{REQUIREMENT_FILE}` (YAML frontmatter followed by a Markdown body).

## Task
1. Read `{REQUIREMENT_FILE}`.
2. Rewrite the Markdown body to be clearer, more complete and well structured. Improve: background/goals, scope, user stories, acceptance criteria, edge cases, non-functional requirements and open questions. Keep the author's intent; do not invent unrelated features.
3. Preserve the YAML frontmatter keys `id`, `phase`, `created_at_ms`. Refresh `updated_at_ms` to the current epoch milliseconds. Keep `id` equal to the directory name and keep `phase` as `collection`.
4. Write the improved requirement back to `{REQUIREMENT_FILE}`.
5. Reply briefly summarizing the improvements you made.

Do not only describe intent — perform the file edits."#,
        requirement_id = requirement_id,
        REQUIREMENT_FILE = REQUIREMENT_FILE,
    );
    vec![
        ToolChatMessage {
            role: "system".to_string(),
            content: system,
        },
        ToolChatMessage {
            role: "user".to_string(),
            content: format!(
                "Optimize requirement **{title}** (`{requirement_id}`) now.\n\n---\n\n{content}"
            ),
        },
    ]
}

/// Dispatches a code-agent task that optimizes the requirement body. A suitable
/// product employee is assigned and the run streams into their conversation.
pub async fn optimize_requirement(
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
    let id = normalize_requirement_id(&id).map_err(map_requirement_err(&headers))?;
    let file_path = requirement_file_path(&workspace, &id);
    if !file_path.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }
    let detail = load_requirement_detail(&workspace, &id)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let Some(employee) = pick_employee_for_role(&workspace, "product") else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "requirement_no_employees"),
        ));
    };

    let task_id = format!("optimize-{id}");
    let _ = create_work_task(
        &workspace,
        CreateWorkTaskParams {
            id: Some(&task_id),
            biz_type: BIZ_TYPE_REQUIREMENT,
            biz_id: &id,
            title: i18n::format_msg(
                crate::agent_locale::resolve_lang_for_workspace(&workspace),
                "optimize_work_task_title",
                &[("requirement_title", detail.title.as_str())],
            )
            .as_str(),
            description: "",
            assignee: Some(&employee.id),
            auto_executable: false,
            metadata: serde_json::json!({ "task_kind": TASK_KIND_OPTIMIZATION }),
        },
    );
    let _ = update_work_task(&workspace, &task_id, |task| {
        task.status = WorkTaskStatus::InProgress;
        Ok(())
    });

    let messages = build_optimize_messages(&id, &detail.title, &detail.content);
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workdir = requirement_dir(&workspace, &id);
    let complete_task_id = task_id.clone();
    spawn_requirement_agent_task(
        &workspace,
        &tools,
        &employee.id,
        AgentTaskSpec {
            kind: TaskKind::RequirementAgent,
            content: format!("Optimize requirement `{id}`"),
            workdir,
            messages,
            context: serde_json::json!({ "requirement_id": id }),
        },
        move |ws| {
            let _ = update_work_task(ws, &complete_task_id, |task| {
                task.status = WorkTaskStatus::Completed;
                task.progress = 100;
                Ok(())
            });
        },
    );

    Ok(Json(AgentDispatchWire::from_employee(&employee)))
}

fn map_requirement_err(
    headers: &HeaderMap,
) -> impl Fn(anyhow::Error) -> (axum::http::StatusCode, String) + '_ {
    move |err| {
        let key = err.to_string();
        let known = [
            "requirement_id_empty",
            "requirement_id_invalid",
            "requirement_title_empty",
            "requirement_not_found",
            "requirement_already_exists",
            "requirement_phase_invalid",
            "requirement_frontmatter_missing",
        ];
        if key == "requirement_not_found" {
            return (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(headers, "requirement_not_found"),
            );
        }
        if known.contains(&key.as_str()) {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(headers, &key),
            );
        }
        (
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(headers, "requirement_id_invalid"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_requirement_md() {
        let meta = RequirementMeta {
            id: "auth-login".to_string(),
            title: "Auth login".to_string(),
            phase: RequirementPhase::Development,
            created_at_ms: 1,
            updated_at_ms: 2,
        };
        let content = "# Goals\n\nSupport SSO.";
        let md = format_requirement_md(&meta, content);
        let (parsed, body) = parse_requirement_md(&md).expect("parse");
        assert_eq!(parsed.id, "auth-login");
        assert_eq!(parsed.phase, RequirementPhase::Development);
        assert_eq!(body, content);
    }

    #[test]
    fn all_phases_are_valid_strings() {
        for phase_str in PHASES {
            let parsed: Result<RequirementPhase, _> = serde_json::from_str(&format!("\"{phase_str}\""));
            assert!(parsed.is_ok());
        }
    }

    #[test]
    fn normalize_requirement_id_trims_and_lowercases() {
        assert_eq!(normalize_requirement_id("  SSO-Login  ").unwrap(), "sso-login");
    }

    #[test]
    fn normalize_requirement_id_rejects_empty() {
        assert_eq!(
            normalize_requirement_id("  ").unwrap_err().to_string(),
            "requirement_id_empty"
        );
    }

    #[test]
    fn normalize_requirement_id_rejects_invalid_chars() {
        assert!(normalize_requirement_id("bad/id").is_err());
    }

    #[test]
    fn derive_requirement_id_from_title() {
        assert_eq!(derive_requirement_id("User Auth Flow").unwrap(), "user-auth-flow");
    }

    #[test]
    fn phase_in_progress_excludes_release() {
        assert!(phase_in_progress(&RequirementPhase::Development));
        assert!(!phase_in_progress(&RequirementPhase::Release));
    }

    #[test]
    fn phase_can_be_updated() {
        let mut meta = RequirementMeta {
            id: "feat-a".to_string(),
            title: "Feature".to_string(),
            phase: RequirementPhase::Collection,
            created_at_ms: 1,
            updated_at_ms: 1,
        };
        meta.phase = RequirementPhase::Development;
        assert_eq!(meta.phase, RequirementPhase::Development);
    }
}
