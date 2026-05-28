pub mod worker;
pub mod task;
pub mod plan;
pub mod store;
pub mod task_graph;
pub mod scheduler;
pub mod executor;
pub mod worker_pool;
pub mod planner;
pub mod events;
pub mod runtime;
pub mod api;
mod legacy;

pub use worker::{Worker, WorkerStatus, LoadLevel};
pub use task::{Task, TaskStatus, TaskPriority, ComplexityLevel, RetryPolicy, TaskContext, TaskResult};
pub use plan::{Plan, PlanStatus, PlanTrigger};
pub use runtime::AutonomousRuntime;

// Backward-compatible re-exports from legacy module
pub use legacy::{
    is_employee_busy,
    is_employee_busy_excluding,
    AutonomyCoordinator,
    AutonomyStatusWire as LegacyAutonomyStatusWire,
    process_autonomy_tick,
    process_employee_autonomy,
    run_autonomy_exploration,
    execute_next_work_task,
    execute_next_todo,
    list_employee_todos_handler,
    run_employee_autonomy_handler,
    run_employee_autonomy_explore_handler,
    get_autonomy_status as legacy_get_autonomy_status,
    run_autonomy_tick_handler as legacy_run_autonomy_tick_handler,
    AutonomyTriggerKind,
    AUTONOMY_INTERVAL_SECS,
    AUTONOMY_DEBOUNCE_MS,
};

// Re-export from tasks module for backward compatibility
pub use crate::tasks::{AgentTaskRecord, CodeAgentTaskParams, TaskKind, TaskRunner, TaskStatus as AgentTaskStatus, TaskStore};
