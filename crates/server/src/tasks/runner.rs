use super::{
    model::{AgentTaskRecord, CodeAgentTaskParams, TaskKind, TaskStatus},
    store::{new_task_id, now_ms, TaskStore},
};
use crate::tools::{
    driver::ToolExecutionResult,
    manager::ToolManager,
    model::ToolInstance,
};
use serde_json::json;
use std::path::Path;

pub struct TaskRunner {
    store: TaskStore,
}

impl TaskRunner {
    pub fn new(workspace: &Path) -> Self {
        Self {
            store: TaskStore::new(workspace),
        }
    }

    pub fn create_parent_task(
        &self,
        kind: TaskKind,
        content: &str,
        workdir: &Path,
        executor_id: Option<&str>,
        context: serde_json::Value,
    ) -> anyhow::Result<AgentTaskRecord> {
        let created = now_ms();
        let id = new_task_id();
        let mut task = AgentTaskRecord::new(
            &CodeAgentTaskParams {
                kind,
                content: content.to_string(),
                workdir: workdir.to_path_buf(),
                messages: vec![],
                executor_id: executor_id.map(str::to_string),
                parent_task_id: None,
                context,
            },
            id,
            created,
        );
        task.status = TaskStatus::Running;
        task.started_at_ms = Some(created);
        self.store.save(&task)?;
        Ok(task)
    }

    pub fn complete_parent_task(&self, task: &mut AgentTaskRecord) -> anyhow::Result<()> {
        task.status = TaskStatus::Completed;
        task.ended_at_ms = Some(now_ms());
        self.store.save(task)?;
        Ok(())
    }

    pub fn fail_parent_task(&self, task: &mut AgentTaskRecord, error: String) -> anyhow::Result<()> {
        task.fail(error, now_ms());
        self.store.save(task)?;
        Ok(())
    }

    pub fn run_code_chat(
        &self,
        tools: &ToolManager,
        params: CodeAgentTaskParams,
    ) -> anyhow::Result<(AgentTaskRecord, ToolInstance, ToolExecutionResult)> {
        let created = now_ms();
        let id = new_task_id();
        let mut task = AgentTaskRecord::new(&params, id, created);
        self.store.save(&task)?;

        let started = now_ms();
        task.mark_running(started);
        self.store.save(&task)?;

        match tools.execute_code_chat(&params.workdir, &params.messages) {
            Ok((instance, result)) => {
                task.complete_with_result(&instance, &result, now_ms());
                self.store.save(&task)?;
                Ok((task, instance, result))
            }
            Err(err) => {
                task.fail(err.to_string(), now_ms());
                self.store.save(&task)?;
                Err(err)
            }
        }
    }

    pub async fn run_code_chat_streaming(
        &self,
        tools: &ToolManager,
        params: CodeAgentTaskParams,
        delta_tx: tokio::sync::mpsc::Sender<String>,
    ) -> anyhow::Result<(AgentTaskRecord, ToolInstance, ToolExecutionResult)> {
        let created = now_ms();
        let id = new_task_id();
        let mut task = AgentTaskRecord::new(&params, id, created);
        self.store.save(&task)?;

        let started = now_ms();
        task.mark_running(started);
        self.store.save(&task)?;

        match tools
            .execute_code_chat_streaming(&params.workdir, &params.messages, delta_tx)
            .await
        {
            Ok((instance, result)) => {
                task.complete_with_result(&instance, &result, now_ms());
                self.store.save(&task)?;
                Ok((task, instance, result))
            }
            Err(err) => {
                task.fail(err.to_string(), now_ms());
                self.store.save(&task)?;
                Err(err)
            }
        }
    }
}

pub fn task_content_from_user_input(input: &str) -> String {
    let trimmed = input.trim();
    super::model::truncate_preview(trimmed, 500)
}

pub fn review_opinion_content(requirement_id: &str, employee_name: &str) -> String {
    format!("Review requirement `{requirement_id}` as {employee_name}")
}

pub fn review_revision_content(requirement_id: &str, employee_name: &str) -> String {
    format!("Revise requirement `{requirement_id}` after review as {employee_name}")
}

pub fn review_summary_content(requirement_id: &str) -> String {
    format!("Summarize review for requirement `{requirement_id}`")
}

pub fn hire_task_content() -> String {
    "Generate new employee profile".to_string()
}

pub fn review_pipeline_content(requirement_id: &str) -> String {
    format!("Run requirement review pipeline for `{requirement_id}`")
}

pub fn review_context(requirement_id: &str) -> serde_json::Value {
    json!({ "requirement_id": requirement_id })
}
