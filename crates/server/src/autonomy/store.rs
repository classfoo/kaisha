use crate::autonomy::task::{Task, TaskStatus};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub fn tasks_root(workspace: &Path) -> PathBuf {
    workspace.join("autonomy").join("tasks")
}

fn task_path(workspace: &Path, task_id: &str) -> PathBuf {
    tasks_root(workspace).join(format!("{task_id}.json"))
}

pub fn save_task(workspace: &Path, task: &Task) -> anyhow::Result<()> {
    let path = task_path(workspace, &task.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_string_pretty(task)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn load_task(workspace: &Path, task_id: &str) -> anyhow::Result<Task> {
    let path = task_path(workspace, task_id);
    let raw = fs::read_to_string(&path)?;
    serde_json::from_str(&raw).map_err(|e| anyhow::anyhow!("load_task: {}", e))
}

pub fn list_tasks(workspace: &Path) -> anyhow::Result<Vec<Task>> {
    let root = tasks_root(workspace);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(task) = fs::read_to_string(&path).and_then(|raw| {
            serde_json::from_str::<Task>(&raw).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        }) {
            items.push(task);
        }
    }
    items.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms));
    Ok(items)
}

pub fn update_task(
    workspace: &Path,
    task_id: &str,
    mut updater: impl FnMut(&mut Task),
) -> anyhow::Result<Task> {
    let mut task = load_task(workspace, task_id)?;
    updater(&mut task);
    save_task(workspace, &task)?;
    Ok(task)
}

pub fn delete_task(workspace: &Path, task_id: &str) -> anyhow::Result<()> {
    let path = task_path(workspace, task_id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn filter_tasks(
    workspace: &Path,
    filter: impl Fn(&Task) -> bool,
) -> anyhow::Result<Vec<Task>> {
    Ok(list_tasks(workspace)?.into_iter().filter(filter).collect())
}

pub fn tasks_by_status(workspace: &Path, status: TaskStatus) -> anyhow::Result<Vec<Task>> {
    filter_tasks(workspace, |t| t.status == status)
}

pub fn tasks_by_assignee(workspace: &Path, assignee: &str) -> anyhow::Result<Vec<Task>> {
    filter_tasks(workspace, |t| t.assignee.as_deref() == Some(assignee))
}

pub fn tasks_by_plan(workspace: &Path, plan_id: &str) -> anyhow::Result<Vec<Task>> {
    filter_tasks(workspace, |t| t.plan_id == plan_id)
}

pub fn active_tasks(workspace: &Path) -> anyhow::Result<Vec<Task>> {
    filter_tasks(workspace, |t| t.status.is_active())
}

pub fn ready_tasks(workspace: &Path) -> anyhow::Result<Vec<Task>> {
    let mut tasks = tasks_by_status(workspace, TaskStatus::Ready)?;
    tasks.sort_by(|a, b| {
        a.priority.cmp(&b.priority)
            .then_with(|| a.created_at_ms.cmp(&b.created_at_ms))
    });
    Ok(tasks)
}

pub fn count_by_status(workspace: &Path) -> anyhow::Result<HashMap<String, usize>> {
    let mut counts = HashMap::new();
    for task in list_tasks(workspace)? {
        *counts.entry(task.status.as_str().to_string()).or_insert(0) += 1;
    }
    Ok(counts)
}
