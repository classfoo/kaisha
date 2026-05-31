use crate::autonomy::events::EventBus;
use crate::autonomy::task::{Task, TaskResult};
use crate::autonomy::worker_pool::WorkerPool;
use crate::tools::manager::ToolManager;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct WorkerHandle {
    pub worker_id: String,
    pub running_tasks: Arc<std::sync::Mutex<HashSet<String>>>,
    pub semaphore: Arc<Semaphore>,
}

impl Clone for WorkerHandle {
    fn clone(&self) -> Self {
        Self {
            worker_id: self.worker_id.clone(),
            running_tasks: self.running_tasks.clone(),
            semaphore: self.semaphore.clone(),
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ExecutorPool {
    handles: Arc<std::sync::Mutex<HashMap<String, WorkerHandle>>>,
    worker_pool: Arc<WorkerPool>,
    event_bus: EventBus,
    tools: Arc<std::sync::RwLock<ToolManager>>,
    workspace: std::path::PathBuf,
}

#[allow(dead_code)]
impl ExecutorPool {
    pub fn new(
        worker_pool: Arc<WorkerPool>,
        event_bus: EventBus,
        tools: Arc<std::sync::RwLock<ToolManager>>,
        workspace: std::path::PathBuf,
    ) -> Self {
        Self {
            handles: Arc::new(std::sync::Mutex::new(HashMap::new())),
            worker_pool,
            event_bus,
            tools,
            workspace,
        }
    }

    pub fn register_worker(&self, worker_id: &str, concurrency: usize) {
        let mut handles = self.handles.lock().expect("executor handles lock poisoned");
        handles.insert(
            worker_id.to_string(),
            WorkerHandle {
                worker_id: worker_id.to_string(),
                running_tasks: Arc::new(std::sync::Mutex::new(HashSet::new())),
                semaphore: Arc::new(Semaphore::new(concurrency)),
            },
        );
    }

    pub async fn execute(&self, worker_id: &str, task: Arc<Task>) -> anyhow::Result<TaskResult> {
        let handle = {
            let handles = self.handles.lock().expect("executor handles lock poisoned");
            handles
                .get(worker_id)
                .ok_or_else(|| anyhow::anyhow!("worker_not_found: {}", worker_id))?
                .clone()
        };

        let permit = handle
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| anyhow::anyhow!("semaphore_acquire_failed: {}", e))?;

        {
            let mut running = handle.running_tasks.lock().expect("running_tasks lock poisoned");
            running.insert(task.id.clone());
        }

        let task_clone = task.clone();
        let workspace = self.workspace.clone();
        let tools = self.tools.clone();

        let result = tokio::task::spawn_blocking(move || {
            Self::run_task_inner(&workspace, &tools, &task_clone)
        })
        .await
        .map_err(|e| anyhow::anyhow!("task_spawn_failed: {}", e))?;

        drop(permit);

        {
            let mut running = handle.running_tasks.lock().expect("running_tasks lock poisoned");
            running.remove(&task.id);
        }

        Ok(result)
    }

    fn run_task_inner(
        workspace: &Path,
        tools: &std::sync::RwLock<ToolManager>,
        task: &Task,
    ) -> TaskResult {
        let _ = workspace;
        let _ = tools;
        let _ = task;
        TaskResult {
            success: true,
            exit_code: 0,
            output_preview: None,
            error: None,
            agent_task_id: None,
        }
    }

    pub async fn cancel_task(&self, _task_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn running_tasks(&self, worker_id: &str) -> usize {
        let handles = self.handles.lock().expect("executor handles lock poisoned");
        if let Some(handle) = handles.get(worker_id) {
            handle
                .running_tasks
                .lock()
                .map(|r| r.len())
                .unwrap_or(0)
        } else {
            0
        }
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}
