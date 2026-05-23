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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::model::{CodeAgentTaskParams, TaskKind, TaskStatus};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-task-runner-{unique}"))
    }

    #[test]
    fn helper_content_builders_include_ids() {
        assert!(hire_task_content().contains("employee"));
        assert!(review_opinion_content("req-1", "Alice").contains("req-1"));
        assert!(review_opinion_content("req-1", "Alice").contains("Alice"));
        assert!(review_revision_content("req-1", "Bob").contains("Revise"));
        assert!(review_summary_content("req-1").contains("req-1"));
        assert!(review_pipeline_content("req-1").contains("req-1"));
        assert_eq!(review_context("req-1")["requirement_id"], "req-1");
    }

    #[test]
    fn task_content_from_user_input_trims_and_truncates() {
        let long = "x".repeat(600);
        let preview = task_content_from_user_input(&format!("  {long}  "));
        assert!(preview.chars().count() <= 501);
        assert!(!preview.starts_with(' '));
    }

    #[test]
    fn create_and_complete_parent_task() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let runner = TaskRunner::new(&workspace);
        let mut parent = runner
            .create_parent_task(
                TaskKind::ReviewPipeline,
                "pipeline",
                &workspace,
                None,
                review_context("r1"),
            )
            .unwrap();
        assert_eq!(parent.status, TaskStatus::Running);
        assert!(parent.started_at_ms.is_some());
        runner.complete_parent_task(&mut parent).unwrap();
        assert_eq!(parent.status, TaskStatus::Completed);
        assert!(parent.ended_at_ms.is_some());
        let loaded = TaskStore::new(&workspace).load(&parent.id).unwrap();
        assert_eq!(loaded.status, TaskStatus::Completed);
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn fail_parent_task_persists_error() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let runner = TaskRunner::new(&workspace);
        let mut parent = runner
            .create_parent_task(
                TaskKind::ReviewPipeline,
                "pipeline",
                &workspace,
                None,
                serde_json::json!({}),
            )
            .unwrap();
        runner
            .fail_parent_task(&mut parent, "review_failed".into())
            .unwrap();
        assert_eq!(parent.status, TaskStatus::Failed);
        assert_eq!(parent.error.as_deref(), Some("review_failed"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn run_code_chat_without_tool_marks_task_failed() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let tools = ToolManager::new(Some(&workspace)).unwrap();
        let runner = TaskRunner::new(&workspace);
        let params = CodeAgentTaskParams {
            kind: TaskKind::RequirementAgent,
            content: "hello".into(),
            workdir: workspace.clone(),
            messages: vec![crate::tools::driver::ToolChatMessage {
                role: "user".into(),
                content: "hi".into(),
            }],
            executor_id: Some("emp-1".into()),
            parent_task_id: None,
            context: serde_json::json!({}),
        };
        let err = runner.run_code_chat(&tools, params).unwrap_err();
        assert!(err.to_string().contains("no_enabled_coding_tool"));
        let tasks = TaskStore::new(&workspace).list().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Failed);
        assert_eq!(tasks[0].executor_id.as_deref(), Some("emp-1"));
        let _ = fs::remove_dir_all(&workspace);
    }
}
