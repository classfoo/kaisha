use crate::autonomy::task::TaskResult;
use crate::autonomy::worker::LoadLevel;
use crate::autonomy::worker::WorkerStatus;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutonomyEvent {
    // Task events
    TaskCreated {
        task_id: String,
        plan_id: String,
    },
    TaskReady {
        task_id: String,
    },
    TaskScheduled {
        task_id: String,
        worker_id: String,
    },
    TaskStarted {
        task_id: String,
    },
    TaskCompleted {
        task_id: String,
        result: TaskResult,
    },
    TaskFailed {
        task_id: String,
        error: String,
    },
    TaskCancelled {
        task_id: String,
    },
    TaskPaused {
        task_id: String,
    },
    TaskResumed {
        task_id: String,
    },

    // Worker events
    WorkerRegistered {
        worker_id: String,
    },
    WorkerStatusChanged {
        worker_id: String,
        from: WorkerStatus,
        to: WorkerStatus,
    },
    WorkerLoadChanged {
        worker_id: String,
        load: LoadLevel,
    },

    // Plan events
    PlanCreated {
        plan_id: String,
    },
    PlanActivated {
        plan_id: String,
    },
    PlanCompleted {
        plan_id: String,
    },

    // System events
    ShopStatusChanged {
        is_open: bool,
    },
}

#[allow(dead_code)]
impl AutonomyEvent {
    pub fn task_id(&self) -> Option<&str> {
        match self {
            Self::TaskCreated { task_id, .. }
            | Self::TaskReady { task_id }
            | Self::TaskScheduled { task_id, .. }
            | Self::TaskStarted { task_id }
            | Self::TaskCompleted { task_id, .. }
            | Self::TaskFailed { task_id, .. }
            | Self::TaskCancelled { task_id }
            | Self::TaskPaused { task_id }
            | Self::TaskResumed { task_id } => Some(task_id),
            _ => None,
        }
    }
}

type EventCallback = Box<dyn Fn(&AutonomyEvent) + Send + Sync>;

#[derive(Clone)]
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<EventCallback>>>,
}

#[allow(dead_code)]
impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn subscribe<F>(&self, callback: F)
    where
        F: Fn(&AutonomyEvent) + Send + Sync + 'static,
    {
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.push(Box::new(callback));
        }
    }

    pub fn publish(&self, event: AutonomyEvent) {
        if let Ok(subs) = self.subscribers.lock() {
            for sub in subs.iter() {
                sub(&event);
            }
        }
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.lock().map(|s| s.len()).unwrap_or(0)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
