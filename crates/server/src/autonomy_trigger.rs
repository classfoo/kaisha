use crate::tasks::{AgentTaskRecord, TaskStatus};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Notify;

pub fn is_employee_busy(tasks: &[AgentTaskRecord], employee_id: &str) -> bool {
    is_employee_busy_excluding(tasks, employee_id, None)
}

pub fn is_employee_busy_excluding(
    tasks: &[AgentTaskRecord],
    employee_id: &str,
    exclude_task_id: Option<&str>,
) -> bool {
    tasks.iter().any(|task| {
        if exclude_task_id == Some(task.id.as_str()) {
            return false;
        }
        task.executor_id.as_deref() == Some(employee_id)
            && matches!(task.status, TaskStatus::Pending | TaskStatus::Running)
    })
}

static AUTONOMY_NOTIFY: OnceLock<Arc<Notify>> = OnceLock::new();

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
