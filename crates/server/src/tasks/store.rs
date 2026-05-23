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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::model::{AgentTaskRecord, CodeAgentTaskParams, TaskKind};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-task-store-{unique}"))
    }

    fn sample_task(id: &str, executor: Option<&str>, parent: Option<&str>) -> AgentTaskRecord {
        AgentTaskRecord::new(
            &CodeAgentTaskParams {
                kind: TaskKind::ReviewOpinion,
                content: id.into(),
                workdir: std::path::PathBuf::from("/tmp"),
                messages: vec![],
                executor_id: executor.map(str::to_string),
                parent_task_id: parent.map(str::to_string),
                context: serde_json::json!({}),
            },
            id.into(),
            now_ms(),
        )
    }

    #[test]
    fn tasks_root_joins_workspace() {
        let root = tasks_root(std::path::Path::new("/ws"));
        assert_eq!(root, std::path::PathBuf::from("/ws/tasks"));
    }

    #[test]
    fn new_task_id_has_prefix() {
        assert!(new_task_id().starts_with("task_"));
    }

    #[test]
    fn load_missing_task_returns_not_found() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let store = TaskStore::new(&workspace);
        let err = store.load("missing").unwrap_err().to_string();
        assert_eq!(err, "task_not_found");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn list_skips_non_json_files() {
        let workspace = temp_workspace();
        let root = tasks_root(&workspace);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("readme.txt"), "x").unwrap();
        let store = TaskStore::new(&workspace);
        store.save(&sample_task("t1", Some("alice"), None)).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn filter_by_parent_task_id_and_limit() {
        let parent = "parent-9";
        let tasks = vec![
            sample_task("c1", Some("a"), Some(parent)),
            sample_task("c2", Some("b"), Some(parent)),
            sample_task("solo", Some("a"), None),
        ];
        let filtered = filter_tasks(
            tasks,
            &TaskListFilter {
                parent_task_id: Some(parent.into()),
                limit: Some(1),
                ..Default::default()
            },
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].parent_task_id.as_deref(), Some(parent));
    }

    #[test]
    fn filter_by_kind() {
        let mut hire = sample_task("h1", None, None);
        hire.kind = TaskKind::EmployeeHire;
        let mut agent = sample_task("a1", Some("e1"), None);
        agent.kind = TaskKind::RequirementAgent;
        let filtered = filter_tasks(
            vec![hire, agent],
            &TaskListFilter {
                kind: Some(TaskKind::EmployeeHire),
                ..Default::default()
            },
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "h1");
    }
}
