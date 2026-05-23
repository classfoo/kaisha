use crate::{i18n, AppState};
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

pub const REQUIREMENT_FILE: &str = "requirement.md";

const PHASES: &[&str] = &[
    "collection",
    "review",
    "confirm",
    "development",
    "testing",
    "release",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequirementPhase {
    Collection,
    Review,
    Confirm,
    Development,
    Testing,
    Release,
}

impl RequirementPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Collection => "collection",
            Self::Review => "review",
            Self::Confirm => "confirm",
            Self::Development => "development",
            Self::Testing => "testing",
            Self::Release => "release",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequirementConfirmStatus {
    Pending,
    Confirmed,
    Abandoned,
}

impl RequirementConfirmStatus {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Abandoned => "abandoned",
        }
    }
}

impl FromStr for RequirementConfirmStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "confirmed" => Ok(Self::Confirmed),
            "abandoned" => Ok(Self::Abandoned),
            _ => Err(format!("unknown confirm status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementMeta {
    pub id: String,
    pub title: String,
    pub phase: RequirementPhase,
    #[serde(default)]
    pub confirm_status: Option<RequirementConfirmStatus>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementSummary {
    pub id: String,
    pub title: String,
    pub phase: RequirementPhase,
    pub confirm_status: Option<RequirementConfirmStatus>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub dir_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementDetail {
    pub id: String,
    pub title: String,
    pub phase: RequirementPhase,
    pub confirm_status: Option<RequirementConfirmStatus>,
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

pub fn archived_requirement_root(workspace: &Path) -> PathBuf {
    workspace.join("requirements").join("archived")
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
        confirm_status: meta.confirm_status,
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
        confirm_status: detail.confirm_status,
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
    let confirm_status = if matches!(phase, RequirementPhase::Confirm) {
        Some(RequirementConfirmStatus::Pending)
    } else {
        None
    };
    let meta = RequirementMeta {
        id: id.clone(),
        title: title.to_string(),
        phase,
        confirm_status,
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
        if matches!(phase, RequirementPhase::Confirm) && meta.confirm_status.is_none() {
            meta.confirm_status = Some(RequirementConfirmStatus::Pending);
        }
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

pub async fn confirm_requirement(
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
    if !matches!(meta.phase, RequirementPhase::Confirm) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "requirement_phase_invalid"),
        ));
    }
    meta.confirm_status = Some(RequirementConfirmStatus::Confirmed);
    meta.phase = RequirementPhase::Development;
    meta.updated_at_ms = now_ms();
    fs::write(&file_path, format_requirement_md(&meta, &content)).map_err(internal_err)?;
    load_requirement_detail(&workspace, &id)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
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
    meta.confirm_status = Some(RequirementConfirmStatus::Abandoned);
    meta.updated_at_ms = now_ms();
    fs::write(&file_path, format_requirement_md(&meta, &content)).map_err(internal_err)?;

    // Move requirement directory to archived directory
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

    // Load summary from archived location
    let summary = load_archived_summary(&workspace, &id)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(RequirementDetail {
        id: summary.id,
        title: summary.title,
        phase: summary.phase,
        confirm_status: summary.confirm_status,
        created_at_ms: summary.created_at_ms,
        updated_at_ms: summary.updated_at_ms,
        dir_path: summary.dir_path,
        content,
        subdirs: Vec::new(),
    }))
}

pub async fn reconfirm_requirement(
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

    // The requirement file is now in the archived directory
    let archived_root = archived_requirement_root(&workspace);
    let archived_dir = archived_root.join(&id);
    let archived_file_path = archived_dir.join(REQUIREMENT_FILE);
    if !archived_file_path.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }

    let raw = fs::read_to_string(&archived_file_path).map_err(internal_err)?;
    let (mut meta, content) =
        parse_requirement_md(&raw).map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                i18n::msg(&headers, "requirement_parse_failed"),
            )
        })?;
    if !matches!(meta.confirm_status, Some(RequirementConfirmStatus::Abandoned)) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "requirement_phase_invalid"),
        ));
    }
    meta.confirm_status = Some(RequirementConfirmStatus::Confirmed);
    meta.phase = RequirementPhase::Development;
    meta.updated_at_ms = now_ms();

    // Move from archived back to active directory
    let active_dir = requirement_dir(&workspace, &id);
    if active_dir.exists() {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "requirement_already_exists"),
        ));
    }
    fs::rename(&archived_dir, &active_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    // Write updated file in active directory
    let active_file_path = active_dir.join(REQUIREMENT_FILE);
    fs::write(&active_file_path, format_requirement_md(&meta, &content)).map_err(internal_err)?;
    load_requirement_detail(&workspace, &id)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
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
        confirm_status: meta.confirm_status,
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

/// Reinstates an archived (abandoned) requirement: moves it back from the archived folder.
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

    // Restore confirm_status to abandoned so it can be reconfirmed
    let file_path = active_dir.join(REQUIREMENT_FILE);
    let raw = fs::read_to_string(&file_path).map_err(internal_err)?;
    let (mut meta, content) = parse_requirement_md(&raw).map_err(|_| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            i18n::msg(&headers, "requirement_parse_failed"),
        )
    })?;
    meta.confirm_status = Some(RequirementConfirmStatus::Abandoned);
    meta.updated_at_ms = now_ms();
    fs::write(&file_path, format_requirement_md(&meta, &content)).map_err(internal_err)?;

    load_requirement_detail(&workspace, &id)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

/// Permanently deletes an archived (abandoned) requirement.
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
            phase: RequirementPhase::Review,
            confirm_status: None,
            created_at_ms: 1,
            updated_at_ms: 2,
        };
        let content = "# Goals\n\nSupport SSO.";
        let md = format_requirement_md(&meta, content);
        let (parsed, body) = parse_requirement_md(&md).expect("parse");
        assert_eq!(parsed.id, "auth-login");
        assert_eq!(parsed.phase, RequirementPhase::Review);
        assert_eq!(body, content);
    }

    #[test]
    fn all_phases_are_valid_strings() {
        for phase_str in PHASES {
            assert!(RequirementPhase::from_str(phase_str).is_some());
        }
    }
}
