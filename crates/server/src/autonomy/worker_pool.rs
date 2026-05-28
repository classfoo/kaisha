use crate::autonomy::events::{AutonomyEvent, EventBus};
use crate::autonomy::worker::{self, LoadLevel, Worker, WorkerStatus};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct WorkerPool {
    workspace: std::path::PathBuf,
    event_bus: EventBus,
    running_counts: Arc<std::sync::Mutex<HashMap<String, usize>>>,
}

impl WorkerPool {
    pub fn new(workspace: std::path::PathBuf, event_bus: EventBus) -> Self {
        Self {
            workspace,
            event_bus,
            running_counts: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub async fn available_workers(&self) -> Vec<Worker> {
        let workers = worker::list_workers(&self.workspace).unwrap_or_default();
        let running = self.running_counts.lock().unwrap();

        workers
            .into_iter()
            .filter(|w| matches!(w.status, WorkerStatus::Active))
            .filter(|w| {
                let count = running.get(&w.id).copied().unwrap_or(0);
                LoadLevel::from_ratio(count, w.concurrency_quota).is_available()
            })
            .collect()
    }

    pub async fn register_worker(&self, worker: &Worker) -> anyhow::Result<()> {
        worker::save_worker(&self.workspace, worker)?;
        self.event_bus.publish(AutonomyEvent::WorkerRegistered {
            worker_id: worker.id.clone(),
        });
        Ok(())
    }

    pub async fn assign_task(&self, worker_id: &str) -> anyhow::Result<()> {
        let mut running = self.running_counts.lock().unwrap();
        let count = running.entry(worker_id.to_string()).or_insert(0);
        *count += 1;

        let current = *count;
        let quota = worker::load_worker(&self.workspace, worker_id)
            .map(|w| w.concurrency_quota)
            .unwrap_or(1);

        let load = LoadLevel::from_ratio(current, quota);
        let _ = worker::update_worker(&self.workspace, worker_id, |w| {
            w.current_load = load;
            if !load.is_available() {
                w.status = WorkerStatus::Busy;
            }
        });

        self.event_bus.publish(AutonomyEvent::WorkerLoadChanged {
            worker_id: worker_id.to_string(),
            load,
        });

        Ok(())
    }

    pub async fn release_task(&self, worker_id: &str, _task_id: &str) -> anyhow::Result<()> {
        let mut running = self.running_counts.lock().unwrap();
        let count = running.entry(worker_id.to_string()).or_insert(0);
        *count = count.saturating_sub(1);

        let current = *count;
        let quota = worker::load_worker(&self.workspace, worker_id)
            .map(|w| w.concurrency_quota)
            .unwrap_or(1);

        let load = LoadLevel::from_ratio(current, quota);
        let _ = worker::update_worker(&self.workspace, worker_id, |w| {
            w.current_load = load;
            if load.is_available() && matches!(w.status, WorkerStatus::Busy) {
                w.status = WorkerStatus::Active;
            }
        });

        self.event_bus.publish(AutonomyEvent::WorkerLoadChanged {
            worker_id: worker_id.to_string(),
            load,
        });

        Ok(())
    }

    pub async fn workers_with_capability(&self, cap: &str) -> Vec<Worker> {
        let workers = worker::list_workers(&self.workspace).unwrap_or_default();
        workers.into_iter().filter(|w| w.has_capability(cap)).collect()
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub fn running_count(&self, worker_id: &str) -> usize {
        self.running_counts
            .lock()
            .unwrap()
            .get(worker_id)
            .copied()
            .unwrap_or(0)
    }

    pub async fn sync_from_employees(&self) -> anyhow::Result<Vec<Worker>> {
        let existing = worker::list_workers(&self.workspace)?;

        if !existing.is_empty() {
            return Ok(existing);
        }

        let employees = crate::employee::list_employee_records(&self.workspace)?;
        let mut workers = Vec::new();

        for emp in &employees {
            let w = Worker {
                id: emp.id.clone(),
                name: emp.name.clone(),
                role: emp.role.clone(),
                capabilities: vec!["general".to_string()],
                concurrency_quota: 2,
                memory_file: Some(format!("shachiku/{}/memory.md", emp.id)),
                status: WorkerStatus::Active,
                current_load: LoadLevel::Idle,
                created_at_ms: worker::now_ms(),
                updated_at_ms: worker::now_ms(),
            };
            worker::save_worker(&self.workspace, &w)?;
            workers.push(w);
        }

        Ok(workers)
    }
}
