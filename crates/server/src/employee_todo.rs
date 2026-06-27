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

fn is_closing_string_quote(bytes: &[u8], mut index: usize) -> bool {
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if ch.is_whitespace() {
            index += 1;
            continue;
        }
        return matches!(ch, ',' | '}' | ']' | ':');
    }
    true
}

/// Repairs common LLM corruption where ASCII quotes are embedded inside JSON strings
/// without escaping, e.g. Chinese text quoting a phrase with `"..."`.
fn repair_unescaped_quotes_in_json(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len() + 64);
    let mut index = 0;
    let mut in_string = false;

    while index < bytes.len() {
        let ch = bytes[index] as char;

        if in_string && ch == '\\' && index + 1 < bytes.len() {
            out.push(ch);
            out.push(bytes[index + 1] as char);
            index += 2;
            continue;
        }

        if ch == '"' {
            if !in_string {
                in_string = true;
                out.push(ch);
            } else if is_closing_string_quote(bytes, index + 1) {
                in_string = false;
                out.push(ch);
            } else {
                out.push('\\');
                out.push(ch);
            }
            index += 1;
            continue;
        }

        out.push(ch);
        index += 1;
    }

    out
}

fn parse_todos_raw(raw: &str, employee_id: &str) -> anyhow::Result<(EmployeeTodoFile, bool)> {
    if let Ok(file) = serde_json::from_str::<EmployeeTodoFile>(raw) {
        return Ok((file, false));
    }
    if let Ok(file) = load_todos_loose(raw, employee_id) {
        return Ok((file, true));
    }
    let repaired = repair_unescaped_quotes_in_json(raw);
    if let Ok(file) = serde_json::from_str::<EmployeeTodoFile>(&repaired) {
        return Ok((file, true));
    }
    Ok((load_todos_loose(&repaired, employee_id)?, true))
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
    let raw = fs::read_to_string(&path)?;
    match parse_todos_raw(&raw, employee_id) {
        Ok((file, needs_rewrite)) => {
            if needs_rewrite {
                let _ = save_todos(workspace, &file);
            }
            Ok(file)
        }
        Err(err) => {
            let backup = path.with_extension("json.bak");
            let _ = fs::copy(&path, &backup);
            tracing::warn!(
                path = ?path,
                backup = ?backup,
                error = %err,
                "failed to parse todos.json; returning empty todo list"
            );
            Ok(EmployeeTodoFile {
                employee_id: employee_id.to_string(),
                items: Vec::new(),
                last_autonomy_run_ms: None,
            })
        }
    }
}

pub fn save_todos(workspace: &Path, file: &EmployeeTodoFile) -> anyhow::Result<()> {
    let path = todo_path(workspace, &file.employee_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, serde_json::to_string_pretty(file)?)?;
    fs::rename(&tmp_path, &path)?;
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

    #[test]
    fn load_todos_repairs_unescaped_quotes_in_description() {
        let workspace = temp_workspace();
        seed_employee(&workspace, "alice");
        let path = todo_path(&workspace, "alice");
        fs::write(
            path,
            r#"{
  "employee_id": "alice",
  "items": [
    {
      "id": "todo_1",
      "title": "Research elevation data",
      "description": "Evaluate sources. Prerequisite for "build terrain pipeline".",
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
        assert!(file.items[0].description.contains("build terrain pipeline"));
        let saved = fs::read_to_string(todo_path(&workspace, "alice")).unwrap();
        assert!(serde_json::from_str::<EmployeeTodoFile>(&saved).is_ok());
        let _ = fs::remove_dir_all(&workspace);
    }
}
