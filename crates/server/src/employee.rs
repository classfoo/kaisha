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
    pub id: String,
    pub name: String,
    pub department: String,
    pub role: String,
    pub memory_file: String,
}

fn workspace_root(state: &AppState) -> Option<PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

pub(crate) fn employee_root(workspace: &Path) -> PathBuf {
    workspace.join("shachiku")
}

pub(crate) fn list_employee_records(workspace: &Path) -> anyhow::Result<Vec<EmployeeRecord>> {
    let root = employee_root(workspace);
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
    Ok(items)
}

pub(crate) fn append_employee_memory(
    workspace: &Path,
    employee_id: &str,
    section_title: &str,
    body: &str,
) -> anyhow::Result<()> {
    let path = employee_root(workspace).join(employee_id).join("memory.md");
    if !path.parent().map(|p| p.exists()).unwrap_or(false) {
        anyhow::bail!("employee_not_found");
    }
    let mut existing = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        String::new()
    };
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&format!("\n## {section_title}\n\n{body}\n"));
    fs::write(path, existing)?;
    Ok(())
}

pub(super) fn ensure_default_employee(workspace: &Path) -> anyhow::Result<()> {
    let root = employee_root(workspace);
    fs::create_dir_all(&root)?;
    ensure_role_employees(workspace)?;
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

/// Seeds one employee per standard role (product, engineering, testing, operations) when missing.
pub(super) fn ensure_role_employees(workspace: &Path) -> anyhow::Result<()> {
    let seeds: [(&str, &str, &str, &str); 4] = [
        ("product-lead", "Product Lead", "product", "产品"),
        ("engineering-lead", "Engineering Lead", "engineering", "研发"),
        ("testing-lead", "Testing Lead", "qa", "测试"),
        ("operations-lead", "Operations Lead", "operations", "运营"),
    ];
    let root = employee_root(workspace);
    for (id, name, department, role) in seeds {
        let dir = root.join(id);
        if dir.exists() {
            continue;
        }
        fs::create_dir_all(&dir)?;
        let profile = EmployeeProfileFile {
            id: id.to_string(),
            name: name.to_string(),
            department: department.to_string(),
            role: role.to_string(),
        };
        fs::write(dir.join("profile.json"), serde_json::to_string_pretty(&profile)?)?;
        fs::write(dir.join("memory.md"), "")?;
    }
    Ok(())
}

pub(crate) fn normalize_employee_id(raw: &str) -> anyhow::Result<String> {
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

/// Removes the employee directory and all associated opinion files across requirements.
pub(super) async fn delete_employee(
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    State(state): State<AppState>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let root = employee_root(&workspace);
    let employee_dir = root.join(&id);
    if !employee_dir.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "employee_not_found"),
        ));
    }

    // Clean up opinion files across all requirements
    let requirements_root = workspace.join("requirements");
    if requirements_root.exists() {
        if let Ok(entries) = fs::read_dir(&requirements_root) {
            for entry in entries.flatten() {
                let req_dir = entry.path();
                if !req_dir.is_dir() {
                    continue;
                }
                let opinion_path = req_dir.join("review").join("opinions").join(format!("{id}.md"));
                if opinion_path.exists() {
                    let _ = fs::remove_file(&opinion_path);
                }
            }
        }
    }

    fs::remove_dir_all(&employee_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok((axum::http::StatusCode::NO_CONTENT, Json(serde_json::json!({}))))
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
