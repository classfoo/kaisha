use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoSource {
    Requirement,
    GitExploration,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmployeeTodoItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TodoStatus,
    pub source: TodoSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement_phase: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmployeeTodoFile {
    pub employee_id: String,
    #[serde(default)]
    pub items: Vec<EmployeeTodoItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_autonomy_run_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct EmployeeTodoItemLoose {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    status: Option<TodoStatus>,
    #[serde(default)]
    source: Option<TodoSource>,
    #[serde(default)]
    requirement_id: Option<String>,
    #[serde(default)]
    requirement_phase: Option<String>,
    #[serde(default)]
    created_at_ms: Option<u64>,
    #[serde(default)]
    updated_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct EmployeeTodoFileLoose {
    #[serde(default)]
    employee_id: Option<String>,
    #[serde(default)]
    items: Vec<EmployeeTodoItemLoose>,
    #[serde(default)]
    last_autonomy_run_ms: Option<u64>,
}

fn normalize_todo_item(loose: EmployeeTodoItemLoose) -> EmployeeTodoItem {
    let ts = now_ms();
    EmployeeTodoItem {
        id: loose
            .id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(new_todo_id),
        title: loose.title,
        description: loose.description,
        status: loose.status.unwrap_or(TodoStatus::Pending),
        source: loose.source.unwrap_or(TodoSource::Manual),
        requirement_id: loose.requirement_id,
        requirement_phase: loose.requirement_phase,
        created_at_ms: loose.created_at_ms.unwrap_or(ts),
        updated_at_ms: loose.updated_at_ms.unwrap_or(ts),
    }
}

fn load_todos_loose(raw: &str, employee_id: &str) -> anyhow::Result<EmployeeTodoFile> {
    let loose: EmployeeTodoFileLoose = serde_json::from_str(raw)?;
    Ok(EmployeeTodoFile {
        employee_id: loose
            .employee_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| employee_id.to_string()),
        items: loose.items.into_iter().map(normalize_todo_item).collect(),
        last_autonomy_run_ms: loose.last_autonomy_run_ms,
    })
}

pub fn todo_path(workspace: &Path, employee_id: &str) -> PathBuf {
    crate::employee::employee_root(workspace)
        .join(employee_id)
        .join("todos.json")
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_todo_id() -> String {
    format!("todo_{}", now_ms())
}

pub fn load_todos(workspace: &Path, employee_id: &str) -> anyhow::Result<EmployeeTodoFile> {
    let path = todo_path(workspace, employee_id);
    if !path.exists() {
        return Ok(EmployeeTodoFile {
            employee_id: employee_id.to_string(),
            items: Vec::new(),
            last_autonomy_run_ms: None,
        });
    }
    let raw = fs::read_to_string(path)?;
    match serde_json::from_str::<EmployeeTodoFile>(&raw) {
        Ok(file) => Ok(file),
        Err(_) => {
            let file = load_todos_loose(&raw, employee_id)?;
            let _ = save_todos(workspace, &file);
            Ok(file)
        }
    }
}

pub fn save_todos(workspace: &Path, file: &EmployeeTodoFile) -> anyhow::Result<()> {
    let path = todo_path(workspace, &file.employee_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(file)?)?;
    Ok(())
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
        std::env::temp_dir().join(format!("kaisha-employee-todo-{unique}"))
    }

    fn seed_employee(workspace: &Path, employee_id: &str) {
        let dir = crate::employee::employee_root(workspace).join(employee_id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("profile.json"),
            serde_json::json!({
                "id": employee_id,
                "name": "Alice",
                "department": "engineering",
                "role": "Engineer"
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn load_missing_todos_returns_empty_file() {
        let workspace = temp_workspace();
        seed_employee(&workspace, "alice");
        let file = load_todos(&workspace, "alice").unwrap();
        assert_eq!(file.employee_id, "alice");
        assert!(file.items.is_empty());
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn load_todos_repairs_items_missing_id() {
        let workspace = temp_workspace();
        seed_employee(&workspace, "alice");
        let path = todo_path(&workspace, "alice");
        fs::write(
            path,
            r#"{
  "employee_id": "alice",
  "items": [
    {
      "title": "Fix bug",
      "description": "Investigate login failure",
      "status": "pending",
      "source": "manual",
      "created_at_ms": 1,
      "updated_at_ms": 1
    }
  ]
}"#,
        )
        .unwrap();
        let file = load_todos(&workspace, "alice").unwrap();
        assert_eq!(file.items.len(), 1);
        assert!(file.items[0].id.starts_with("todo_"));
        let reloaded = load_todos(&workspace, "alice").unwrap();
        assert_eq!(reloaded.items[0].id, file.items[0].id);
        let _ = fs::remove_dir_all(&workspace);
    }
}
