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
    Ok(serde_json::from_str(&raw)?)
}

pub fn save_todos(workspace: &Path, file: &EmployeeTodoFile) -> anyhow::Result<()> {
    let path = todo_path(workspace, &file.employee_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(file)?)?;
    Ok(())
}

pub fn incomplete_todos(file: &EmployeeTodoFile) -> Vec<&EmployeeTodoItem> {
    file.items
        .iter()
        .filter(|item| matches!(item.status, TodoStatus::Pending | TodoStatus::InProgress))
        .collect()
}

pub fn count_incomplete_todos(file: &EmployeeTodoFile) -> usize {
    incomplete_todos(file).len()
}

pub fn next_pending_todo(file: &EmployeeTodoFile) -> Option<&EmployeeTodoItem> {
    file.items
        .iter()
        .find(|item| item.status == TodoStatus::Pending)
}

pub fn add_todo(
    workspace: &Path,
    employee_id: &str,
    title: &str,
    description: &str,
    source: TodoSource,
    requirement_id: Option<&str>,
    requirement_phase: Option<&str>,
) -> anyhow::Result<EmployeeTodoItem> {
    let mut file = load_todos(workspace, employee_id)?;
    let ts = now_ms();
    let item = EmployeeTodoItem {
        id: new_todo_id(),
        title: title.trim().to_string(),
        description: description.trim().to_string(),
        status: TodoStatus::Pending,
        source,
        requirement_id: requirement_id.map(str::to_string),
        requirement_phase: requirement_phase.map(str::to_string),
        created_at_ms: ts,
        updated_at_ms: ts,
    };
    file.items.push(item.clone());
    save_todos(workspace, &file)?;
    Ok(item)
}

pub fn mark_todo_status(
    workspace: &Path,
    employee_id: &str,
    todo_id: &str,
    status: TodoStatus,
) -> anyhow::Result<EmployeeTodoItem> {
    let mut file = load_todos(workspace, employee_id)?;
    let item = file
        .items
        .iter_mut()
        .find(|item| item.id == todo_id)
        .ok_or_else(|| anyhow::anyhow!("todo_not_found"))?;
    item.status = status;
    item.updated_at_ms = now_ms();
    let updated = item.clone();
    save_todos(workspace, &file)?;
    Ok(updated)
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
    fn add_and_count_incomplete_todos() {
        let workspace = temp_workspace();
        seed_employee(&workspace, "alice");
        add_todo(
            &workspace,
            "alice",
            "Review auth spec",
            "Check acceptance criteria",
            TodoSource::Requirement,
            Some("auth"),
            Some("review"),
        )
        .unwrap();
        let file = load_todos(&workspace, "alice").unwrap();
        assert_eq!(count_incomplete_todos(&file), 1);
        mark_todo_status(&workspace, "alice", &file.items[0].id, TodoStatus::Completed).unwrap();
        let file = load_todos(&workspace, "alice").unwrap();
        assert_eq!(count_incomplete_todos(&file), 0);
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn next_pending_todo_skips_in_progress() {
        let workspace = temp_workspace();
        seed_employee(&workspace, "bob");
        add_todo(&workspace, "bob", "A", "a", TodoSource::Manual, None, None).unwrap();
        let mut file = load_todos(&workspace, "bob").unwrap();
        file.items[0].status = TodoStatus::InProgress;
        save_todos(&workspace, &file).unwrap();
        add_todo(&workspace, "bob", "B", "b", TodoSource::Manual, None, None).unwrap();
        let file = load_todos(&workspace, "bob").unwrap();
        assert_eq!(next_pending_todo(&file).map(|t| t.title.as_str()), Some("B"));
        let _ = fs::remove_dir_all(&workspace);
    }
}
