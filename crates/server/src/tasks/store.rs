use super::model::{AgentTaskRecord, TaskKind, TaskStatus};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn tasks_root(workspace: &Path) -> PathBuf {
    workspace.join("tasks")
}

fn task_path(workspace: &Path, task_id: &str) -> PathBuf {
    tasks_root(workspace).join(format!("{task_id}.json"))
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_task_id() -> String {
    format!("task_{}", now_ms())
}

pub struct TaskStore {
    workspace: PathBuf,
}

impl TaskStore {
    pub fn new(workspace: &Path) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
        }
    }

    pub fn save(&self, task: &AgentTaskRecord) -> anyhow::Result<()> {
        let root = tasks_root(&self.workspace);
        fs::create_dir_all(&root)?;
        let path = task_path(&self.workspace, &task.id);
        fs::write(path, serde_json::to_string_pretty(task)?)?;
        Ok(())
    }

    pub fn load(&self, task_id: &str) -> anyhow::Result<AgentTaskRecord> {
        let path = task_path(&self.workspace, task_id);
        if !path.exists() {
            anyhow::bail!("task_not_found");
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn list(&self) -> anyhow::Result<Vec<AgentTaskRecord>> {
        let root = tasks_root(&self.workspace);
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
            let raw = fs::read_to_string(&path)?;
            if let Ok(task) = serde_json::from_str::<AgentTaskRecord>(&raw) {
                items.push(task);
            }
        }
        items.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));
        Ok(items)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskListFilter {
    pub executor_id: Option<String>,
    pub status: Option<TaskStatus>,
    pub kind: Option<TaskKind>,
    pub parent_task_id: Option<String>,
    pub limit: Option<usize>,
}

pub fn filter_tasks(tasks: Vec<AgentTaskRecord>, filter: &TaskListFilter) -> Vec<AgentTaskRecord> {
    let mut items: Vec<AgentTaskRecord> = tasks
        .into_iter()
        .filter(|t| {
            if let Some(ref eid) = filter.executor_id {
                if t.executor_id.as_deref() != Some(eid.as_str()) {
                    return false;
                }
            }
            if let Some(status) = filter.status {
                if t.status != status {
                    return false;
                }
            }
            if let Some(kind) = filter.kind {
                if t.kind != kind {
                    return false;
                }
            }
            if let Some(ref pid) = filter.parent_task_id {
                if t.parent_task_id.as_deref() != Some(pid.as_str()) {
                    return false;
                }
            }
            true
        })
        .collect();
    items.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));
    if let Some(limit) = filter.limit {
        items.truncate(limit);
    }
    items
}
