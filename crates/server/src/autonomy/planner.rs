use crate::autonomy::plan::{self, Plan, PlanStatus, PlanTrigger};
use crate::autonomy::task::{
    ComplexityLevel, RetryPolicy, Task, TaskContext, TaskPriority, TaskStatus,
};
use crate::autonomy::store;
use crate::autonomy::task_graph::TaskGraph;
use crate::tools::manager::ToolManager;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct Planner {
    workspace: std::path::PathBuf,
    tools: Arc<std::sync::RwLock<ToolManager>>,
}

pub struct PlanningContext {
    pub title: String,
    pub description: String,
    pub trigger: PlanTrigger,
    pub tasks: Vec<TaskSpec>,
}

pub struct TaskSpec {
    pub id: Option<String>,
    pub title: String,
    pub description: String,
    pub priority: TaskPriority,
    pub complexity: ComplexityLevel,
    pub dependencies: Vec<String>,
    pub context: TaskContext,
    pub retry_policy: Option<RetryPolicy>,
}

impl Planner {
    pub fn new(
        workspace: std::path::PathBuf,
        tools: Arc<std::sync::RwLock<ToolManager>>,
    ) -> Self {
        Self { workspace, tools }
    }

    pub fn create_plan(&self, ctx: PlanningContext) -> anyhow::Result<(Plan, TaskGraph)> {
        let plan_id = plan::new_plan_id();
        let now = plan::now_ms();

        let mut plan = Plan {
            id: plan_id.clone(),
            title: ctx.title,
            description: ctx.description,
            status: PlanStatus::Active,
            trigger: ctx.trigger,
            recurrence: None,
            root_tasks: Vec::new(),
            all_tasks: Vec::new(),
            progress_percent: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            total_tasks: ctx.tasks.len(),
            created_at_ms: now,
            started_at_ms: Some(now),
            completed_at_ms: None,
        };

        let mut graph = TaskGraph::new();

        for spec in &ctx.tasks {
            let task_id = spec.id.clone().unwrap_or_else(|| {
                format!("{}_{}", plan_id, crate::autonomy::task::now_ms())
            });

            let task = Task {
                id: task_id.clone(),
                title: spec.title.clone(),
                description: spec.description.clone(),
                status: TaskStatus::Pending,
                priority: spec.priority,
                complexity: spec.complexity,
                estimated_duration_secs: Self::estimate_duration(spec.complexity),
                actual_duration_secs: None,
                assignee: None,
                executor_pid: None,
                dependencies: spec.dependencies.clone(),
                dependents: Vec::new(),
                plan_id: plan_id.clone(),
                parent_task_id: None,
                sub_tasks: Vec::new(),
                context: spec.context.clone(),
                result: None,
                retry_policy: spec.retry_policy.clone().unwrap_or_default(),
                retry_count: 0,
                next_retry_at_ms: None,
                created_at_ms: now,
                started_at_ms: None,
                completed_at_ms: None,
            };

            if task.dependencies.is_empty() {
                plan.root_tasks.push(task_id.clone());
            }
            plan.all_tasks.push(task_id.clone());

            store::save_task(&self.workspace, &task)?;
            graph.add_task(task)?;
        }

        plan::save_plan(&self.workspace, &plan)?;
        graph.persist(&self.workspace)?;

        Ok((plan, graph))
    }

    pub fn replan(
        &self,
        plan_id: &str,
        failed_task_id: &str,
        new_specs: Vec<TaskSpec>,
    ) -> anyhow::Result<Plan> {
        let mut plan = plan::load_plan(&self.workspace, plan_id)?;

        for spec in new_specs {
            let task_id = spec.id.clone().unwrap_or_else(|| {
                format!("{}_{}", plan_id, crate::autonomy::task::now_ms())
            });
            let now = plan::now_ms();

            let task = Task {
                id: task_id.clone(),
                title: spec.title.clone(),
                description: spec.description.clone(),
                status: TaskStatus::Pending,
                priority: spec.priority,
                complexity: spec.complexity,
                estimated_duration_secs: Self::estimate_duration(spec.complexity),
                actual_duration_secs: None,
                assignee: None,
                executor_pid: None,
                dependencies: spec.dependencies.clone(),
                dependents: Vec::new(),
                plan_id: plan_id.to_string(),
                parent_task_id: Some(failed_task_id.to_string()),
                sub_tasks: Vec::new(),
                context: spec.context.clone(),
                result: None,
                retry_policy: spec.retry_policy.clone().unwrap_or_default(),
                retry_count: 0,
                next_retry_at_ms: None,
                created_at_ms: now,
                started_at_ms: None,
                completed_at_ms: None,
            };

            store::save_task(&self.workspace, &task)?;
            plan.all_tasks.push(task_id);
            plan.total_tasks += 1;
        }

        plan::save_plan(&self.workspace, &plan)?;
        Ok(plan)
    }

    fn estimate_duration(complexity: ComplexityLevel) -> u64 {
        match complexity {
            ComplexityLevel::Trivial => 300,
            ComplexityLevel::Simple => 1800,
            ComplexityLevel::Moderate => 7200,
            ComplexityLevel::Complex => 28800,
            ComplexityLevel::VeryComplex => 86400,
        }
    }
}
