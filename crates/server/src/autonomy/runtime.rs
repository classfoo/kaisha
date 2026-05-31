use crate::autonomy::events::EventBus;
use crate::autonomy::executor::ExecutorPool;
use crate::autonomy::planner::Planner;
use crate::autonomy::scheduler::Scheduler;
use crate::autonomy::store;
use crate::autonomy::task::TaskStatus;
use crate::autonomy::task_graph::TaskGraph;
use crate::autonomy::worker_pool::WorkerPool;
use crate::tools::manager::ToolManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Clone)]
#[allow(dead_code)]
pub struct AutonomousRuntime {
    pub workspace: std::path::PathBuf,
    pub task_graph: Arc<tokio::sync::RwLock<TaskGraph>>,
    pub worker_pool: Arc<WorkerPool>,
    pub scheduler: Arc<Scheduler>,
    pub executor: Arc<ExecutorPool>,
    pub planner: Arc<Planner>,
    pub event_bus: EventBus,
    pub tools: Arc<std::sync::RwLock<ToolManager>>,
    notify: Arc<Notify>,
}

impl AutonomousRuntime {
    pub fn new(
        workspace: std::path::PathBuf,
        tools: Arc<std::sync::RwLock<ToolManager>>,
    ) -> Self {
        let event_bus = EventBus::new();
        let worker_pool = Arc::new(WorkerPool::new(workspace.clone(), event_bus.clone()));
        let task_graph = Arc::new(tokio::sync::RwLock::new(TaskGraph::new()));
        let scheduler = Arc::new(
            Scheduler::new(
                task_graph.clone(),
                worker_pool.clone(),
                event_bus.clone(),
                workspace.clone(),
            )
        );
        let executor = Arc::new(ExecutorPool::new(
            worker_pool.clone(),
            event_bus.clone(),
            tools.clone(),
            workspace.clone(),
        ));
        let planner = Arc::new(Planner::new(workspace.clone(), tools.clone()));

        Self {
            workspace,
            task_graph,
            worker_pool,
            scheduler,
            executor,
            planner,
            event_bus,
            tools,
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn initialize(&self) -> anyhow::Result<()> {
        let workers = self.worker_pool.sync_from_employees().await?;
        for worker in &workers {
            self.executor.register_worker(&worker.id, worker.concurrency_quota);
        }

        let loaded_graph = TaskGraph::load(&self.workspace).unwrap_or_else(|_| TaskGraph::new());
        {
            let mut graph = self.task_graph.write().await;
            *graph = loaded_graph;
        }

        self.restore_pending_tasks().await?;

        tracing::info!("AutonomousRuntime initialized with {} workers", workers.len());
        Ok(())
    }

    async fn restore_pending_tasks(&self) -> anyhow::Result<()> {
        let pending = store::filter_tasks(&self.workspace, |t| {
            t.status == TaskStatus::Running || t.status == TaskStatus::Scheduled
        })?;

        for task in pending {
            store::update_task(&self.workspace, &task.id, |t| {
                t.status = TaskStatus::Ready;
                t.assignee = None;
            })?;

            let mut graph = self.task_graph.write().await;
            if let Some(t) = graph.get_task_mut(&task.id) {
                t.status = TaskStatus::Ready;
                t.assignee = None;
            }
        }

        Ok(())
    }

    pub async fn run_loop(self: Arc<Self>) {
        let scheduler = self.scheduler.clone();
        tokio::spawn(async move {
            scheduler.run().await;
        });

        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                _ = self.notify.notified() => {}
            }

            if let Err(err) = self.tick().await {
                tracing::warn!("autonomy runtime tick failed: {err}");
            }
        }
    }

    async fn tick(&self) -> anyhow::Result<()> {
        if let Ok(status) = crate::shop_status::load_shop_status(&self.workspace) {
            if !status.is_open {
                return Ok(());
            }
        }

        while let Some((task_id, worker_id)) = self.scheduler.schedule_next().await? {
            self.scheduler.on_task_started(&task_id).await?;
            self.worker_pool.assign_task(&worker_id).await?;

            let task = store::load_task(&self.workspace, &task_id)?;
            let executor = self.executor.clone();
            let scheduler = self.scheduler.clone();
            let task_id_clone = task_id.clone();
            let worker_id_clone = worker_id.clone();
            let task_arc = Arc::new(task);

            tokio::spawn(async move {
                match executor.execute(&worker_id_clone, task_arc).await {
                    Ok(result) => {
                        let _ = scheduler.on_task_completed(&task_id_clone, &result).await;
                    }
                    Err(err) => {
                        let _ = scheduler.on_task_failed(&task_id_clone, &err.to_string()).await;
                    }
                }
            });
        }

        self.tick_retry_tasks().await?;

        Ok(())
    }

    async fn tick_retry_tasks(&self) -> anyhow::Result<()> {
        let now = crate::autonomy::task::now_ms();
        let retryable = store::filter_tasks(&self.workspace, |t| {
            t.status == TaskStatus::Failed
                && t.is_retryable()
                && t.next_retry_at_ms.map(|at| at <= now).unwrap_or(false)
        })?;

        for task in retryable {
            store::update_task(&self.workspace, &task.id, |t| {
                t.status = TaskStatus::Ready;
                t.next_retry_at_ms = None;
            })?;

            let mut graph = self.task_graph.write().await;
            if let Some(t) = graph.get_task_mut(&task.id) {
                t.status = TaskStatus::Ready;
            }
        }

        Ok(())
    }

    pub fn notify(&self) {
        self.notify.notify_one();
    }

    pub async fn list_tasks(&self) -> anyhow::Result<Vec<crate::autonomy::task::Task>> {
        store::list_tasks(&self.workspace)
    }

    pub async fn list_plans(&self) -> anyhow::Result<Vec<crate::autonomy::plan::Plan>> {
        crate::autonomy::plan::list_plans(&self.workspace)
    }

    pub async fn list_workers(&self) -> anyhow::Result<Vec<crate::autonomy::worker::Worker>> {
        crate::autonomy::worker::list_workers(&self.workspace)
    }
}
