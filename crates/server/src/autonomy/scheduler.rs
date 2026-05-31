use crate::autonomy::events::AutonomyEvent;
use crate::autonomy::events::EventBus;
use crate::autonomy::task::{Task, TaskPriority, TaskStatus};
use crate::autonomy::task_graph::{ReadyTask, TaskGraph};
use crate::autonomy::worker_pool::WorkerPool;
use crate::autonomy::store;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub tick_interval_ms: u64,
    pub enable_preemption: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            tick_interval_ms: 100,
            enable_preemption: true,
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct Scheduler {
    task_graph: Arc<tokio::sync::RwLock<TaskGraph>>,
    worker_pool: Arc<WorkerPool>,
    event_bus: EventBus,
    config: SchedulerConfig,
    notify: Arc<Notify>,
    workspace: std::path::PathBuf,
}

#[allow(dead_code)]
impl Scheduler {
    pub fn new(
        task_graph: Arc<tokio::sync::RwLock<TaskGraph>>,
        worker_pool: Arc<WorkerPool>,
        event_bus: EventBus,
        workspace: std::path::PathBuf,
    ) -> Self {
        Self {
            task_graph,
            worker_pool,
            event_bus,
            config: SchedulerConfig::default(),
            notify: Arc::new(Notify::new()),
            workspace,
        }
    }

    pub fn with_config(mut self, config: SchedulerConfig) -> Self {
        self.config = config;
        self
    }

    pub fn notify(&self) {
        self.notify.notify_one();
    }

    pub async fn run(&self) {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(self.config.tick_interval_ms)) => {}
                _ = self.notify.notified() => {}
            }
            if let Err(err) = self.tick().await {
                tracing::warn!("scheduler tick failed: {err}");
            }
        }
    }

    async fn tick(&self) -> anyhow::Result<()> {
        let graph = self.task_graph.read().await;

        if graph.ready_queue_len() == 0 {
            return Ok(());
        }

        Ok(())
    }

    pub async fn schedule_next(&self) -> anyhow::Result<Option<(String, String)>> {
        let mut graph = self.task_graph.write().await;
        let available_workers = self.worker_pool.available_workers().await;

        while let Some(ready_task) = graph.pop_ready() {
            let task = graph.get_task(&ready_task.task_id).cloned();
            let Some(task) = task else { continue };

            let worker = self.select_worker(&task, &available_workers);
            let Some(worker) = worker else {
                if self.config.enable_preemption && ready_task.priority == TaskPriority::Critical {
                    if let Some(preempted) = self.try_preempt(&ready_task, &mut graph).await? {
                        let worker_id = preempted;
                        graph.schedule_task(&ready_task.task_id, &worker_id)?;
                        self.event_bus.publish(AutonomyEvent::TaskScheduled {
                            task_id: ready_task.task_id.clone(),
                            worker_id: worker_id.clone(),
                        });
                        return Ok(Some((ready_task.task_id, worker_id)));
                    }
                }
                graph.push_ready(ready_task);
                return Ok(None);
            };

            graph.schedule_task(&ready_task.task_id, &worker.id)?;
            let task_id_clone = ready_task.task_id.clone();
            let worker_id_clone = worker.id.clone();
            self.event_bus.publish(AutonomyEvent::TaskScheduled {
                task_id: task_id_clone,
                worker_id: worker_id_clone,
            });

            return Ok(Some((ready_task.task_id, worker.id)));
        }

        Ok(None)
    }

    fn select_worker(
        &self,
        _task: &Task,
        available: &[crate::autonomy::worker::Worker],
    ) -> Option<crate::autonomy::worker::Worker> {
        if available.is_empty() {
            return None;
        }

        let capable: Vec<_> = available
            .iter()
            .filter(|w| w.has_capability("general"))
            .collect();

        if capable.is_empty() {
            return available.first().cloned();
        }

        capable
            .into_iter()
            .min_by(|a, b| a.current_load.cmp(&b.current_load))
            .cloned()
    }

    async fn try_preempt(
        &self,
        _critical_task: &ReadyTask,
        _graph: &mut TaskGraph,
    ) -> anyhow::Result<Option<String>> {
        let running_tasks = store::filter_tasks(&self.workspace, |t| t.status.is_active())?;
        let low_priority = running_tasks
            .iter()
            .filter(|t| t.priority == TaskPriority::Low)
            .next();

        if let Some(target) = low_priority {
            let worker_id = target.assignee.clone();
            if let Some(wid) = worker_id {
                store::update_task(&self.workspace, &target.id, |t| {
                    t.status = TaskStatus::Paused;
                })?;
                self.event_bus.publish(AutonomyEvent::TaskPaused {
                    task_id: target.id.clone(),
                });
                self.worker_pool.release_task(&wid, &target.id).await?;
                return Ok(Some(wid));
            }
        }

        Ok(None)
    }

    pub async fn on_task_completed(&self, task_id: &str, result: &crate::autonomy::task::TaskResult) -> anyhow::Result<()> {
        let mut graph = self.task_graph.write().await;
        let newly_ready = graph.complete_task(task_id)?;

        store::update_task(&self.workspace, task_id, |t| {
            t.status = TaskStatus::Completed;
            t.result = Some(result.clone());
            t.completed_at_ms = Some(crate::autonomy::task::now_ms());
        })?;

        self.event_bus.publish(AutonomyEvent::TaskCompleted {
            task_id: task_id.to_string(),
            result: result.clone(),
        });

        let task = graph.get_task(task_id).cloned();
        if let Some(task) = task {
            if let Some(assignee) = &task.assignee {
                self.worker_pool.release_task(assignee, task_id).await?;
            }
        }

        for dep_id in newly_ready {
            self.event_bus.publish(AutonomyEvent::TaskReady {
                task_id: dep_id.clone(),
            });
            store::update_task(&self.workspace, &dep_id, |t| {
                t.status = TaskStatus::Ready;
            })?;
        }

        Ok(())
    }

    pub async fn on_task_failed(&self, task_id: &str, error: &str) -> anyhow::Result<()> {
        let mut graph = self.task_graph.write().await;
        graph.fail_task(task_id, error)?;

        let should_retry = {
            let task = store::update_task(&self.workspace, task_id, |t| {
                t.status = TaskStatus::Failed;
                t.result = Some(crate::autonomy::task::TaskResult {
                    success: false,
                    exit_code: 1,
                    output_preview: None,
                    error: Some(error.to_string()),
                    agent_task_id: None,
                });
            })?;
            task.is_retryable()
        };

        self.event_bus.publish(AutonomyEvent::TaskFailed {
            task_id: task_id.to_string(),
            error: error.to_string(),
        });

        let task = store::load_task(&self.workspace, task_id)?;
        if let Some(assignee) = &task.assignee {
            self.worker_pool.release_task(assignee, task_id).await?;
        }

        if should_retry {
            let delay = task.retry_policy.next_delay(task.retry_count);
            let retry_at = crate::autonomy::task::now_ms() + delay;
            store::update_task(&self.workspace, task_id, |t| {
                t.retry_count += 1;
                t.next_retry_at_ms = Some(retry_at);
                t.status = TaskStatus::Pending;
            })?;
        }

        Ok(())
    }

    pub async fn on_task_started(&self, task_id: &str) -> anyhow::Result<()> {
        store::update_task(&self.workspace, task_id, |t| {
            t.status = TaskStatus::Running;
            t.started_at_ms = Some(crate::autonomy::task::now_ms());
        })?;
        self.event_bus.publish(AutonomyEvent::TaskStarted {
            task_id: task_id.to_string(),
        });
        Ok(())
    }

    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}
