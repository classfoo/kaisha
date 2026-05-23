use crate::{i18n, tasks::{CodeAgentTaskParams, TaskKind, TaskRunner, hire_task_content}, AppState};
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
use crate::tools::driver::ToolChatMessage;

/// Agent-generated employee profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentEmployeeProfile {
    name: String,
    department: String,
    role: String,
}

/// Parse agent output to extract employee profile.
/// The agent is asked to output JSON, but we also try to parse free-form text as fallback.
fn parse_agent_employee_output(output: &str) -> Option<AgentEmployeeProfile> {
    // Try to find a JSON block
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') {
            if let Ok(profile) = serde_json::from_str::<AgentEmployeeProfile>(trimmed) {
                if !profile.name.trim().is_empty()
                    && !profile.department.trim().is_empty()
                    && !profile.role.trim().is_empty()
                {
                    return Some(AgentEmployeeProfile {
                        name: profile.name.trim().to_string(),
                        department: profile.department.trim().to_string(),
                        role: profile.role.trim().to_string(),
                    });
                }
            }
        }
    }

    // Try to parse the entire output as JSON
    if let Ok(profile) = serde_json::from_str::<AgentEmployeeProfile>(output) {
        if !profile.name.trim().is_empty()
            && !profile.department.trim().is_empty()
            && !profile.role.trim().is_empty()
        {
            return Some(AgentEmployeeProfile {
                name: profile.name.trim().to_string(),
                department: profile.department.trim().to_string(),
                role: profile.role.trim().to_string(),
            });
        }
    }

    // Fallback: look for key-value pairs in text
    let mut name = String::new();
    let mut department = String::new();
    let mut role = String::new();
    for line in output.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if let Some(rest) = trimmed.strip_prefix("Name:") {
            name = rest.trim().to_string();
        } else if let Some(rest) = trimmed.strip_prefix("Department:") {
            department = rest.trim().to_string();
        } else if let Some(rest) = trimmed.strip_prefix("Role:") {
            role = rest.trim().to_string();
        }
    }
    if !name.is_empty() && !department.is_empty() && !role.is_empty() {
        return Some(AgentEmployeeProfile { name, department, role });
    }

    None
}

fn build_hire_employee_prompt(existing_count: usize) -> String {
    format!(
        r#"You are creating a new team member for a software development company. There are already {existing_count} employees in the company.

Create a new employee with a realistic and culturally appropriate name, a reasonable department, and a specific role. The employee should have a unique identity that complements the existing team.

Requirements:
- Name: Give the employee a realistic human name (not "Employee N" or generic)
- Department: Choose from: product, engineering, qa, operations, design, marketing, finance, hr, legal, sales, support
- Role: Define a specific job title (e.g., "Senior Frontend Developer", "UX Designer", "QA Automation Engineer")

Respond with ONLY a valid JSON object in this exact format:
{{"name": "Full Name", "department": "department_name", "role": "Specific Role"}}

Do not include any other text, explanation, or markdown code fences. Only the JSON object."#,
        existing_count = existing_count
    )
}

pub(super) async fn hire_employee(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<EmployeeRecord>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };

    // Get current employee count for context
    let existing_count = list_employee_records(&workspace)
        .map(|v| v.len())
        .unwrap_or(0);

    // Invoke the code agent to generate employee profile
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let task_runner = TaskRunner::new(&workspace);

    let messages = vec![
        ToolChatMessage {
            role: "system".to_string(),
            content: "You are an HR assistant that creates new employee profiles for a software development company. You output only valid JSON objects with name, department, and role fields.".to_string(),
        },
        ToolChatMessage {
            role: "user".to_string(),
            content: build_hire_employee_prompt(existing_count),
        },
    ];

    let (_task, _instance, result) = task_runner
        .run_code_chat(
            &tools,
            CodeAgentTaskParams {
                kind: TaskKind::EmployeeHire,
                content: hire_task_content(),
                workdir: workspace.clone(),
                messages,
                executor_id: None,
                parent_task_id: None,
                context: serde_json::json!({ "existing_employee_count": existing_count }),
            },
        )
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, format!("agent_failed: {}", e)))?;

    if result.exit_code != 0 {
        return Err((
            axum::http::StatusCode::BAD_GATEWAY,
            i18n::msg(&headers, "employee_agent_failed"),
        ));
    }

    let profile = parse_agent_employee_output(&result.output).ok_or_else(|| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            i18n::msg(&headers, "employee_agent_invalid_output"),
        )
    })?;

    // Create the employee using the agent-generated profile
    let name = profile.name.trim();
    let department = profile.department.trim();
    let role = profile.role.trim();

    let employee_id = derive_employee_id(name).map_err(|err| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, &err.to_string()),
        )
    })?;

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

    let profile_file = EmployeeProfileFile {
        id: employee_id.clone(),
        name: name.to_string(),
        department: department.to_string(),
        role: role.to_string(),
    };
    fs::write(
        employee_dir.join("profile.json"),
        serde_json::to_string_pretty(&profile_file)
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

pub(crate) fn archived_employee_root(workspace: &Path) -> PathBuf {
    workspace.join("shachiku").join("archived")
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

/// Ensures the employee directory exists. No seed employees are created here;
/// employees should be created via the hire_employee (agent-based) path.
pub(super) fn ensure_default_employee(workspace: &Path) -> anyhow::Result<()> {
    let root = employee_root(workspace);
    fs::create_dir_all(&root)?;
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

/// Fires an employee: moves the employee directory to the archived folder
/// and cleans up opinion files across all requirements.
pub(super) async fn fire_employee(
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
    let archived_root = archived_employee_root(&workspace);
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

    // Move employee to archived directory
    fs::create_dir_all(&archived_root)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let archived_dir = archived_root.join(&id);
    fs::rename(&employee_dir, &archived_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok((axum::http::StatusCode::NO_CONTENT, Json(serde_json::json!({}))))
}

/// Lists archived (fired) employees from the archived directory.
pub(super) async fn list_archived_employees(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Vec<EmployeeRecord>>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let root = archived_employee_root(&workspace);
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

/// Reinstates a fired employee: moves the employee directory back from the archived folder.
pub(super) async fn reinstate_employee(
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    State(state): State<AppState>,
) -> Result<Json<EmployeeRecord>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let root = employee_root(&workspace);
    let archived_root = archived_employee_root(&workspace);
    let archived_dir = archived_root.join(&id);
    if !archived_dir.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "employee_archived_not_found"),
        ));
    }

    // Move back to active directory
    let employee_dir = root.join(&id);
    if employee_dir.exists() {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "employee_already_exists"),
        ));
    }
    fs::rename(&archived_dir, &employee_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    // Parse and return the reinstated employee
    let profile_path = employee_dir.join("profile.json");
    let item = parse_employee(&id, &profile_path)
        .map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "employee_profile_corrupt".to_string()))?;
    Ok(Json(item))
}

/// Permanently deletes an archived employee, including all their data.
pub(super) async fn hard_delete_employee(
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
    let archived_root = archived_employee_root(&workspace);
    let archived_dir = archived_root.join(&id);
    if !archived_dir.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "employee_archived_not_found"),
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

    fs::remove_dir_all(&archived_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok((axum::http::StatusCode::NO_CONTENT, Json(serde_json::json!({}))))
}

/// Hands over an archived employee's data: moves memory and conversation files to a transfer directory.
pub(super) async fn handover_employee(
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
    let archived_root = archived_employee_root(&workspace);
    let archived_dir = archived_root.join(&id);
    if !archived_dir.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "employee_archived_not_found"),
        ));
    }

    // Create handover directory at workspace/handovers/{employee_id}
    let handover_root = workspace.join("handovers");
    let handover_dir = handover_root.join(&id);
    fs::create_dir_all(&handover_dir)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    // Copy memory.md and conversation.json to handover directory
    for file_name in ["memory.md", "conversation.json", "profile.json"] {
        let src = archived_dir.join(file_name);
        let dst = handover_dir.join(file_name);
        if src.exists() {
            let _ = fs::copy(&src, &dst);
        }
    }

    // Archive the employee directory (remove archived copy)
    fs::remove_dir_all(&archived_dir)
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
