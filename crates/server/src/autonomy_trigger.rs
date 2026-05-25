use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Notify;

static AUTONOMY_NOTIFY: OnceLock<Arc<Notify>> = OnceLock::new();

pub fn register_autonomy_notify(notify: Arc<Notify>) {
    let _ = AUTONOMY_NOTIFY.set(notify);
}

pub fn wake_autonomy_loop() {
    if let Some(notify) = AUTONOMY_NOTIFY.get() {
        notify.notify_one();
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn autonomy_root(workspace: &Path) -> PathBuf {
    workspace.join("autonomy")
}

fn pending_trigger_path(workspace: &Path, employee_id: &str) -> PathBuf {
    autonomy_root(workspace)
        .join("pending")
        .join(format!("{employee_id}.trigger"))
}

pub fn mark_employee_for_autonomy(workspace: &Path, employee_id: &str) -> anyhow::Result<()> {
    let path = pending_trigger_path(workspace, employee_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, now_ms().to_string())?;
    wake_autonomy_loop();
    Ok(())
}

pub fn list_pending_autonomy_employees(workspace: &Path) -> anyhow::Result<Vec<String>> {
    let dir = autonomy_root(workspace).join("pending");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".trigger") {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}

pub fn clear_pending_autonomy(workspace: &Path, employee_id: &str) -> anyhow::Result<()> {
    let path = pending_trigger_path(workspace, employee_id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_and_list_pending_autonomy_employees() {
        let workspace = std::env::temp_dir().join(format!(
            "kaisha-autonomy-trigger-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        mark_employee_for_autonomy(&workspace, "alice").unwrap();
        let pending = list_pending_autonomy_employees(&workspace).unwrap();
        assert_eq!(pending, vec!["alice".to_string()]);
        clear_pending_autonomy(&workspace, "alice").unwrap();
        assert!(list_pending_autonomy_employees(&workspace).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&workspace);
    }
}
