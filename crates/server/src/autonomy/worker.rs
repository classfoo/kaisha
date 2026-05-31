#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Active,
    Busy,
    Paused,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum LoadLevel {
    Idle,
    Light,
    Moderate,
    Heavy,
    Critical,
}

impl LoadLevel {
    pub fn from_ratio(running: usize, quota: usize) -> Self {
        if quota == 0 {
            return Self::Critical;
        }
        let ratio = running as f64 / quota as f64;
        if ratio == 0.0 {
            Self::Idle
        } else if ratio <= 0.4 {
            Self::Light
        } else if ratio <= 0.7 {
            Self::Moderate
        } else if ratio <= 0.9 {
            Self::Heavy
        } else {
            Self::Critical
        }
    }

    pub fn is_available(&self) -> bool {
        !matches!(self, Self::Critical)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id: String,
    pub name: String,
    pub role: String,
    pub capabilities: Vec<String>,
    pub concurrency_quota: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_file: Option<String>,
    pub status: WorkerStatus,
    pub current_load: LoadLevel,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl Worker {
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.is_empty() || self.capabilities.iter().any(|c| c == cap)
    }
}

pub fn workers_root(workspace: &Path) -> PathBuf {
    workspace.join("autonomy").join("workers")
}

fn worker_path(workspace: &Path, worker_id: &str) -> PathBuf {
    workers_root(workspace).join(format!("{worker_id}.json"))
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_worker_id() -> String {
    format!("worker_{}", now_ms())
}

pub fn save_worker(workspace: &Path, worker: &Worker) -> anyhow::Result<()> {
    let path = worker_path(workspace, &worker.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(worker)?)?;
    Ok(())
}

pub fn load_worker(workspace: &Path, worker_id: &str) -> anyhow::Result<Worker> {
    let path = worker_path(workspace, worker_id);
    let raw = fs::read_to_string(&path)?;
    serde_json::from_str(&raw).map_err(|e| anyhow::anyhow!("load_worker: {}", e))
}

pub fn list_workers(workspace: &Path) -> anyhow::Result<Vec<Worker>> {
    let root = workers_root(workspace);
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
        if let Ok(worker) = fs::read_to_string(&path).and_then(|raw| {
            serde_json::from_str::<Worker>(&raw).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        }) {
            items.push(worker);
        }
    }
    items.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms));
    Ok(items)
}

pub fn update_worker(
    workspace: &Path,
    worker_id: &str,
    mut updater: impl FnMut(&mut Worker),
) -> anyhow::Result<Worker> {
    let mut worker = load_worker(workspace, worker_id)?;
    updater(&mut worker);
    worker.updated_at_ms = now_ms();
    save_worker(workspace, &worker)?;
    Ok(worker)
}

pub fn delete_worker(workspace: &Path, worker_id: &str) -> anyhow::Result<()> {
    let path = worker_path(workspace, worker_id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_level_calculation() {
        assert!(matches!(LoadLevel::from_ratio(0, 2), LoadLevel::Idle));
        assert!(matches!(LoadLevel::from_ratio(1, 2), LoadLevel::Moderate));
        assert!(matches!(LoadLevel::from_ratio(2, 2), LoadLevel::Critical));
    }
}
