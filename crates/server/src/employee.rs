use crate::{i18n, AppState};
use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CreateEmployeeRequest {
    name: String,
    department: String,
    role: String,
    id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmployeeProfileFile {
    id: String,
    name: String,
    department: String,
    role: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct EmployeeRecord {
    id: String,
    name: String,
    department: String,
    role: String,
    memory_file: String,
}

fn workspace_root(state: &AppState) -> Option<PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

fn employee_root(workspace: &Path) -> PathBuf {
    workspace.join("shachiku")
}

pub(super) fn ensure_default_employee(workspace: &Path) -> anyhow::Result<()> {
    let root = employee_root(workspace);
    fs::create_dir_all(&root)?;
    let employee_id = "employee-1";
    let employee_dir = root.join(employee_id);
    if employee_dir.exists() {
        return Ok(());
    }

    fs::create_dir_all(&employee_dir)?;
    let profile = EmployeeProfileFile {
        id: employee_id.to_string(),
        name: "Employee 1".to_string(),
        department: "default".to_string(),
        role: "default".to_string(),
    };
    fs::write(
        employee_dir.join("profile.json"),
        serde_json::to_string_pretty(&profile)?,
    )?;
    fs::write(employee_dir.join("memory.md"), "")?;
    Ok(())
}

fn normalize_employee_id(raw: &str) -> anyhow::Result<String> {
    let trimmed = raw.trim().to_lowercase();
    if trimmed.is_empty() {
        anyhow::bail!("employee_id_empty");
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        anyhow::bail!("employee_id_invalid");
    }
    Ok(trimmed)
}

fn derive_employee_id(name: &str) -> anyhow::Result<String> {
    let slug: String = name
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
        anyhow::bail!("employee_id_invalid");
    }
    Ok(compact)
}

fn parse_employee(dir_name: &str, profile_path: &Path) -> anyhow::Result<EmployeeRecord> {
    let raw = fs::read_to_string(profile_path)?;
    let profile: EmployeeProfileFile = serde_json::from_str(&raw)?;
    Ok(EmployeeRecord {
        id: profile.id,
        name: profile.name,
        department: profile.department,
        role: profile.role,
        memory_file: format!("shachiku/{dir_name}/memory.md"),
    })
}

pub(super) async fn list_employees(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Vec<EmployeeRecord>>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };

    let root = employee_root(&workspace);
    if !root.exists() {
        return Ok(Json(Vec::new()));
    }

    let mut items = Vec::new();
    let entries =
        fs::read_dir(&root).map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let profile_path = path.join("profile.json");
        if !profile_path.exists() {
            continue;
        }
        if let Ok(item) = parse_employee(&dir_name, &profile_path) {
            items.push(item);
        }
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Json(items))
}

pub(super) async fn create_employee(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<CreateEmployeeRequest>,
) -> Result<Json<EmployeeRecord>, (axum::http::StatusCode, String)> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "employee_name_empty"),
        ));
    }
    let department = payload.department.trim();
    if department.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "employee_department_empty"),
        ));
    }
    let role = payload.role.trim();
    if role.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "employee_role_empty"),
        ));
    }

    let employee_id = match payload.id {
        Some(custom) => normalize_employee_id(&custom),
        None => derive_employee_id(name),
    }
    .map_err(|err| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, &err.to_string()),
        )
    })?;

    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let root = employee_root(&workspace);
    fs::create_dir_all(&root)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let employee_dir = root.join(&employee_id);
    if employee_dir.exists() {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "employee_already_exists"),
        ));
    }

    fs::create_dir_all(&employee_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let profile = EmployeeProfileFile {
        id: employee_id.clone(),
        name: name.to_string(),
        department: department.to_string(),
        role: role.to_string(),
    };
    fs::write(
        employee_dir.join("profile.json"),
        serde_json::to_string_pretty(&profile)
            .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?,
    )
    .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    fs::write(employee_dir.join("memory.md"), "")
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(Json(EmployeeRecord {
        id: employee_id.clone(),
        name: name.to_string(),
        department: department.to_string(),
        role: role.to_string(),
        memory_file: format!("shachiku/{employee_id}/memory.md"),
    }))
}
