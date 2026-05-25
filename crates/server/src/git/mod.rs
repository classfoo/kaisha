mod registry;
mod service;

pub(crate) use registry::{list_repos, repo_dir, MAIN_REPO_ID};

use crate::{i18n, AppState};
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    Json,
};
use registry::{
    add_repo, ensure_main_repo, find_repo, validate_repo_id, validate_repo_name,
    GitRepoRecord,
};
use serde::{Deserialize, Serialize};
use service::{execute_operation, repo_status, GitCommandOutput, GitOperation, GitRepoStatus};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct GitRepoWire {
    pub id: String,
    pub name: String,
    pub is_main: bool,
    pub path: String,
    pub created_at_ms: u64,
    pub initialized: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListReposResponse {
    pub repos: Vec<GitRepoWire>,
    pub main_repo_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRepoRequest {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoDetailResponse {
    pub repo: GitRepoWire,
    pub status: Option<GitRepoStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitOperationRequest {
    #[serde(flatten)]
    pub operation: GitOperation,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct InitGitProjectRequest {
    pub project: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct InitGitProjectResponse {
    pub project: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ExecGitCommandRequest {
    pub project: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ExecGitCommandResponse {
    pub stdout: String,
}

fn workspace_root(state: &AppState) -> Option<PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

fn map_repo(workspace: &std::path::Path, record: GitRepoRecord) -> GitRepoWire {
    let dir = repo_dir(workspace, &record.id);
    GitRepoWire {
        id: record.id,
        name: record.name,
        is_main: record.is_main,
        path: dir.to_string_lossy().to_string(),
        created_at_ms: record.created_at_ms,
        initialized: dir.join(".git").exists(),
    }
}

fn map_err(headers: &HeaderMap, err: anyhow::Error) -> (axum::http::StatusCode, String) {
    let key = err.to_string();
    let known = [
        "git_repo_id_empty",
        "git_repo_id_invalid",
        "git_repo_name_empty",
        "git_repo_not_found",
        "git_repo_already_exists",
        "git_args_empty",
        "git_config_injection_denied",
        "git_not_initialized",
        "git_project_empty",
        "git_project_invalid",
        "git_project_not_found",
    ];
    if key == "git_repo_not_found" || key == "git_project_not_found" {
        return (
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(headers, "git_repo_not_found"),
        );
    }
    if known.contains(&key.as_str()) {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(headers, &key),
        );
    }
    (axum::http::StatusCode::BAD_REQUEST, key)
}

pub fn ensure_workspace_repos(workspace: &std::path::Path) -> anyhow::Result<()> {
    ensure_main_repo(workspace)?;
    Ok(())
}

pub async fn list_git_repos(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<ListReposResponse>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    ensure_main_repo(&workspace).map_err(|e| map_err(&headers, e))?;
    let repos = list_repos(&workspace).map_err(|e| map_err(&headers, e))?;
    Ok(Json(ListReposResponse {
        main_repo_id: MAIN_REPO_ID.to_string(),
        repos: repos
            .into_iter()
            .map(|r| map_repo(&workspace, r))
            .collect(),
    }))
}

pub async fn create_git_repo(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<CreateRepoRequest>,
) -> Result<Json<RepoDetailResponse>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let name = validate_repo_name(&payload.name).map_err(|e| map_err(&headers, e))?;
    let id = if let Some(raw) = payload.id {
        validate_repo_id(&raw).map_err(|e| map_err(&headers, e))?
    } else {
        slugify_repo_id(&name)
    };
    let record = add_repo(&workspace, &id, &name, false).map_err(|e| map_err(&headers, e))?;
    let dir = repo_dir(&workspace, &id);
    std::fs::create_dir_all(&dir).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    if !dir.join(".git").exists() {
        service::git_init(&dir).map_err(|e| map_err(&headers, e))?;
    }
    let wire = map_repo(&workspace, record);
    let status = repo_status(&dir).ok();
    Ok(Json(RepoDetailResponse { repo: wire, status }))
}

pub async fn get_git_repo(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(repo_id): AxumPath<String>,
) -> Result<Json<RepoDetailResponse>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let record = find_repo(&workspace, &repo_id).map_err(|e| map_err(&headers, e))?;
    let dir = repo_dir(&workspace, &record.id);
    let wire = map_repo(&workspace, record);
    let status = if wire.initialized {
        repo_status(&dir).ok()
    } else {
        None
    };
    Ok(Json(RepoDetailResponse { repo: wire, status }))
}

pub async fn run_git_operation(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(repo_id): AxumPath<String>,
    Json(payload): Json<GitOperationRequest>,
) -> Result<Json<GitCommandOutput>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let record = find_repo(&workspace, &repo_id).map_err(|e| map_err(&headers, e))?;
    let dir = repo_dir(&workspace, &record.id);
    if !dir.exists() {
        return Err(map_err(
            &headers,
            anyhow::anyhow!("git_repo_not_found"),
        ));
    }

    let output = if matches!(payload.operation, GitOperation::Clone { .. }) {
        run_clone(&workspace, &record.id, &payload.operation).map_err(|e| map_err(&headers, e))?
    } else {
        if !dir.join(".git").exists() {
            service::git_init(&dir).map_err(|e| map_err(&headers, e))?;
        }
        execute_operation(&dir, &payload.operation).map_err(|e| map_err(&headers, e))?
    };
    Ok(Json(output))
}

fn run_clone(
    workspace: &std::path::Path,
    repo_id: &str,
    op: &GitOperation,
) -> anyhow::Result<GitCommandOutput> {
    let GitOperation::Clone { url, directory } = op else {
        anyhow::bail!("git_invalid_operation");
    };
    let parent = repo_dir(workspace, repo_id);
    std::fs::create_dir_all(&parent)?;
    let target_name = directory
        .clone()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| "clone".to_string());
    let target = parent.join(&target_name);
    if target.exists() {
        anyhow::bail!("git_clone_target_exists");
    }
    let args = vec![
        "clone".to_string(),
        url.clone(),
        target_name,
    ];
    service::run_git(&parent, &args)
}

fn slugify_repo_id(name: &str) -> String {
    let slug: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        return format!("repo-{}", chrono_lite_ms());
    }
    trimmed.to_string()
}

fn chrono_lite_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// Legacy endpoints (project name maps to repo id under repos/)

fn validate_project_name(raw: &str) -> anyhow::Result<String> {
    validate_repo_id(raw)
}

pub(super) fn init_repository(workspace: &std::path::Path, project: &str) -> anyhow::Result<String> {
    ensure_main_repo(workspace)?;
    let id = validate_project_name(project)?;
    let name = id.clone();
    if find_repo(workspace, &id).is_err() {
        let _ = add_repo(workspace, &id, &name, id == MAIN_REPO_ID);
    }
    let dir = repo_dir(workspace, &id);
    std::fs::create_dir_all(&dir)?;
    if !dir.join(".git").exists() {
        service::git_init(&dir)?;
    }
    Ok(String::new())
}

pub(super) fn exec_git_command(
    workspace: &std::path::Path,
    project: &str,
    args: &[String],
) -> anyhow::Result<String> {
    let id = validate_project_name(project)?;
    let dir = repo_dir(workspace, &id);
    if !dir.exists() {
        anyhow::bail!("git_repo_not_found");
    }
    if !dir.join(".git").exists() {
        service::git_init(&dir)?;
    }
    let result = service::run_git(&dir, args)?;
    if result.exit_code == 0 {
        return Ok(result.stdout.trim().to_string());
    }
    let msg = if result.stderr.trim().is_empty() {
        result.stdout.trim().to_string()
    } else {
        result.stderr.trim().to_string()
    };
    if msg.is_empty() {
        anyhow::bail!("git_command_failed");
    }
    anyhow::bail!(msg)
}

pub(super) async fn init_git_project(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<InitGitProjectRequest>,
) -> Result<Json<InitGitProjectResponse>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };

    let project = validate_project_name(&payload.project).map_err(|e| map_err(&headers, e))?;

    init_repository(&workspace, &project).map_err(|e| map_err(&headers, e))?;
    let path = repo_dir(&workspace, &project).to_string_lossy().to_string();
    Ok(Json(InitGitProjectResponse { project, path }))
}

pub(super) async fn exec_git(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<ExecGitCommandRequest>,
) -> Result<Json<ExecGitCommandResponse>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };

    let project = validate_project_name(&payload.project).map_err(|e| map_err(&headers, e))?;

    if payload.args.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "git_args_empty"),
        ));
    }

    let stdout = exec_git_command(&workspace, &project, &payload.args).map_err(|e| map_err(&headers, e))?;
    Ok(Json(ExecGitCommandResponse { stdout }))
}

#[cfg(test)]
mod tests {
    use super::{exec_git_command, init_repository};
    use crate::git::registry::{repo_dir, MAIN_REPO_ID};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }

    #[test]
    fn init_repository_creates_project_with_git_dir() {
        let workspace = unique_temp_dir("kaisha-git-init");
        fs::create_dir_all(&workspace).expect("failed to create workspace");

        let result = init_repository(&workspace, "demo-project");
        assert!(result.is_ok(), "init should succeed");

        let project = repo_dir(&workspace, "demo-project");
        assert!(project.join(".git").exists(), "git metadata should exist");
    }

    #[test]
    fn exec_git_command_runs_status_inside_project() {
        let workspace = unique_temp_dir("kaisha-git-exec");
        fs::create_dir_all(&workspace).expect("failed to create workspace");
        init_repository(&workspace, "demo-project").expect("init should succeed");

        let args = vec!["status".to_string(), "--short".to_string()];
        let output = exec_git_command(&workspace, "demo-project", &args).expect("command should succeed");
        assert!(output.is_empty(), "new repo should have clean short status");
    }

    #[test]
    fn ensure_main_on_init() {
        let workspace = unique_temp_dir("kaisha-git-main-init");
        fs::create_dir_all(&workspace).expect("failed to create workspace");
        init_repository(&workspace, MAIN_REPO_ID).expect("init main");
        assert!(repo_dir(&workspace, MAIN_REPO_ID).join(".git").exists());
    }
}
