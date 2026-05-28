use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Draft,
    Active,
    Executing,
    Paused,
    Completed,
    Cancelled,
}

impl PlanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Executing => "executing",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanTrigger {
    Manual,
    Scheduled,
    EventDriven,
    Dependency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recurrence {
    pub interval_secs: u64,
    #[serde(default)]
    pub max_runs: Option<u32>,
    #[serde(default)]
    pub runs_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: PlanStatus,
    pub trigger: PlanTrigger,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<Recurrence>,

    pub root_tasks: Vec<String>,
    pub all_tasks: Vec<String>,

    pub progress_percent: u8,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub total_tasks: usize,

    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<u64>,
}

impl Plan {
    pub fn update_progress(&mut self) {
        if self.total_tasks == 0 {
            self.progress_percent = 0;
            return;
        }
        self.progress_percent = ((self.completed_tasks as f64 / self.total_tasks as f64) * 100.0) as u8;
        if self.completed_tasks + self.failed_tasks == self.total_tasks {
            self.status = if self.failed_tasks == 0 {
                PlanStatus::Completed
            } else {
                PlanStatus::Completed
            };
            self.completed_at_ms = Some(now_ms());
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_plan_id() -> String {
    format!("plan_{}", now_ms())
}

pub fn plans_root(workspace: &std::path::Path) -> std::path::PathBuf {
    workspace.join("autonomy").join("plans")
}

fn plan_path(workspace: &std::path::Path, plan_id: &str) -> std::path::PathBuf {
    plans_root(workspace).join(format!("{plan_id}.json"))
}

pub fn save_plan(workspace: &std::path::Path, plan: &Plan) -> anyhow::Result<()> {
    let path = plan_path(workspace, &plan.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(plan)?)?;
    Ok(())
}

pub fn load_plan(workspace: &std::path::Path, plan_id: &str) -> anyhow::Result<Plan> {
    let path = plan_path(workspace, plan_id);
    let raw = std::fs::read_to_string(&path)?;
    serde_json::from_str(&raw).map_err(|e| anyhow::anyhow!("load_plan: {}", e))
}

pub fn list_plans(workspace: &std::path::Path) -> anyhow::Result<Vec<Plan>> {
    let root = plans_root(workspace);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(plan) = std::fs::read_to_string(&path).and_then(|raw| {
            serde_json::from_str::<Plan>(&raw).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        }) {
            items.push(plan);
        }
    }
    items.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_progress() {
        let mut plan = Plan {
            id: "test".into(),
            title: "Test".into(),
            description: "".into(),
            status: PlanStatus::Executing,
            trigger: PlanTrigger::Manual,
            recurrence: None,
            root_tasks: vec![],
            all_tasks: vec!["a".into(), "b".into(), "c".into()],
            progress_percent: 0,
            completed_tasks: 2,
            failed_tasks: 0,
            total_tasks: 3,
            created_at_ms: 0,
            started_at_ms: None,
            completed_at_ms: None,
        };
        plan.update_progress();
        assert_eq!(plan.progress_percent, 66);
    }
}
