use crate::autonomy_trigger::{is_employee_busy, is_employee_busy_excluding, mark_employee_for_autonomy};
use super::{
    model::{AgentTaskRecord, CodeAgentTaskParams, StreamingExecutionFailure, TaskKind, TaskStatus},
    runtime::task_runtime_handle,
    store::{new_task_id, now_ms, TaskStore},
};
use crate::tools::{
    driver::{ChatStreamEvent, ToolExecutionResult},
    manager::ToolManager,
    model::ToolInstance,
};
use serde_json::json;
use std::path::Path;

async fn emit_streaming_failure(event_tx: &tokio::sync::mpsc::Sender<ChatStreamEvent>, message: &str) {
    let _ = event_tx
        .send(ChatStreamEvent::Result {
            summary: Some(message.to_string()),
            model: None,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            is_error: true,
        })
        .await;
}

fn streaming_failure(task_id: &str, message: String) -> anyhow::Error {
    StreamingExecutionFailure {
        task_id: task_id.to_string(),
        message,
    }
    .into()
}

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

    pub fn stop_task(&self, task_id: &str) -> anyhow::Result<AgentTaskRecord> {
        let mut task = self.store.load(task_id)?;
        if !can_stop_task(&task) {
            anyhow::bail!("task_cannot_stop");
        }
        task_runtime_handle().request_stop(task_id);
        task.cancel(now_ms());
        self.store.save(&task)?;
        task_runtime_handle().unregister(task_id);
        Ok(task)
    }

    pub fn stop_all_tasks_for_executor(&self, executor_id: &str) -> anyhow::Result<Vec<AgentTaskRecord>> {
        let tasks = self.store.list()?;
        let stoppable_ids: Vec<String> = tasks
            .into_iter()
            .filter(|t| t.executor_id.as_deref() == Some(executor_id))
            .filter(|t| can_stop_task(t))
            .map(|t| t.id)
            .collect();
        let mut stopped = Vec::new();
        for task_id in stoppable_ids {
            stopped.push(self.stop_task(&task_id)?);
        }
        Ok(stopped)
    }

    /// Checks shop status and returns `true` if the shop is closed,
    /// signaling that the task should be queued rather than failed.
    fn shop_is_closed(&self) -> bool {
        if let Ok(status) = crate::shop_status::load_shop_status(self.store.workspace()) {
            !status.is_open
        } else {
            false
        }
    }

    fn execute_code_chat_inner(
        &self,
        tools: &ToolManager,
        params: &CodeAgentTaskParams,
        task: &mut AgentTaskRecord,
    ) -> anyhow::Result<(ToolInstance, ToolExecutionResult)> {
        let lang = crate::agent_locale::resolve_lang_for_workspace(self.store.workspace());
        let messages =
            crate::agent_locale::ensure_language_system_message(params.messages.clone(), lang);
        let runtime = task_runtime_handle();

        // Retry on transient errors like no_enabled_coding_tool (tool might be initializing)
        const MAX_RETRIES: u32 = 2;
        const RETRY_DELAY_MS: u64 = 500;
        let mut last_err = None;

        for attempt in 0..=MAX_RETRIES {
            match tools.execute_code_chat_for_task(
                &params.workdir,
                &messages,
                &task.id,
                runtime.as_ref(),
            ) {
                Ok(result) => return Ok(result),
                Err(err) => {
                    let msg = err.to_string();
                    if msg.contains("task_cancelled") {
                        if let Ok(current) = self.store.load(&task.id) {
                            if current.status == TaskStatus::Cancelled {
                                runtime.unregister(&task.id);
                                anyhow::bail!("task_cancelled");
                            }
                        }
                        task.cancel(now_ms());
                        self.store.save(task)?;
                        runtime.unregister(&task.id);
                        anyhow::bail!("task_cancelled");
                    }
                    // Only retry for no_enabled_coding_tool (transient - tool config might be loading)
                    if msg.contains("no_enabled_coding_tool") && attempt < MAX_RETRIES {
                        tracing::warn!(attempt = attempt, "no coding tool available, retrying...");
                        std::thread::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS));
                        last_err = Some(err);
                        continue;
                    }
                    last_err = Some(err);
                    break;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("execute_failed")))
    }

    fn finalize_task_success(
        &self,
        tools: &ToolManager,
        params: &CodeAgentTaskParams,
        task: &mut AgentTaskRecord,
        instance: &ToolInstance,
        result: &ToolExecutionResult,
    ) -> anyhow::Result<()> {
        if let Ok(current) = self.store.load(&task.id) {
            if current.status == TaskStatus::Cancelled {
                task_runtime_handle().unregister(&task.id);
                if let Some(executor_id) = task.executor_id.as_deref() {
                    self.try_drain_queued_reruns(tools, executor_id)?;
                }
                return Ok(());
            }
        }
        task.complete_with_result(instance, result, now_ms());
        self.store.save(task)?;
        if !result.output.is_empty() {
            let _ = self.store.save_output(&task.id, &result.output);
        }
        // Sync WorkTask status for autonomy tasks
        if matches!(params.kind, TaskKind::AutonomyExplore | TaskKind::AutonomyExecute | TaskKind::WorkTaskExecute) {
            let _ = crate::autonomy_task::sync_work_task_status(
                self.store.workspace(),
                &task.id,
                &task.status,
            );
        }
        self.notify_autonomy_if_needed(&params.kind, task);
        task_runtime_handle().unregister(&task.id);
        if let Some(executor_id) = task.executor_id.as_deref() {
            self.try_drain_queued_reruns(tools, executor_id)?;
        }
        Ok(())
    }

    fn finalize_task_error(
        &self,
        tools: &ToolManager,
        params: &CodeAgentTaskParams,
        task: &mut AgentTaskRecord,
        error: String,
    ) -> anyhow::Result<()> {
        if let Ok(current) = self.store.load(&task.id) {
            if current.status == TaskStatus::Cancelled {
                task_runtime_handle().unregister(&task.id);
                if let Some(executor_id) = task.executor_id.as_deref() {
                    self.try_drain_queued_reruns(tools, executor_id)?;
                }
                return Ok(());
            }
        }
        task.fail(error, now_ms());
        self.store.save(task)?;
        // Sync WorkTask status for autonomy tasks
        if matches!(params.kind, TaskKind::AutonomyExplore | TaskKind::AutonomyExecute | TaskKind::WorkTaskExecute) {
            let _ = crate::autonomy_task::sync_work_task_status(
                self.store.workspace(),
                &task.id,
                &task.status,
            );
        }
        self.notify_autonomy_if_needed(&params.kind, task);
        task_runtime_handle().unregister(&task.id);
        if let Some(executor_id) = task.executor_id.as_deref() {
            self.try_drain_queued_reruns(tools, executor_id)?;
        }
        Ok(())
    }

    pub fn queue_rerun(&self, task_id: &str) -> anyhow::Result<AgentTaskRecord> {
        let mut task = self.store.load(task_id)?;
        if !task.is_terminal() && !task.is_queued_for_rerun() {
            anyhow::bail!("task_cannot_queue_rerun");
        }
        task.mark_queued_rerun(now_ms());
        self.store.save(&task)?;
        Ok(task)
    }

    fn next_queued_rerun_task_id(&self, executor_id: &str) -> anyhow::Result<Option<String>> {
        let mut queued = self
            .store
            .list()?
            .into_iter()
            .filter(|task| {
                task.executor_id.as_deref() == Some(executor_id) && task.is_queued_for_rerun()
            })
            .collect::<Vec<_>>();
        queued.sort_by_key(|task| task.queued_rerun_at_ms());
        Ok(queued.first().map(|task| task.id.clone()))
    }

    pub fn try_drain_queued_reruns(
        &self,
        tools: &ToolManager,
        executor_id: &str,
    ) -> anyhow::Result<()> {
        loop {
            if is_employee_busy(&self.store.list()?, executor_id) {
                return Ok(());
            }
            let Some(task_id) = self.next_queued_rerun_task_id(executor_id)? else {
                return Ok(());
            };
            let task = self.store.load(&task_id)?;
            let params = build_rerun_params(&task);
            match self.rerun_code_chat(tools, &task_id, params) {
                Ok(_) => continue,
                Err(err) if err.to_string().contains("task_cancelled") => return Ok(()),
                Err(err) => return Err(err),
            }
        }
    }

    pub fn run_code_chat(
        &self,
        tools: &ToolManager,
        params: CodeAgentTaskParams,
    ) -> anyhow::Result<(AgentTaskRecord, ToolInstance, ToolExecutionResult)> {
        // If shop is closed, create task as pending instead of failing immediately.
        // The task will be picked up when the shop reopens via queued rerun drain.
        if self.shop_is_closed() {
            let created = now_ms();
            let id = new_task_id();
            let mut task = AgentTaskRecord::new(&params, id, created);
            task.status = TaskStatus::Pending;
            self.store.save(&task)?;
            anyhow::bail!("shop_is_closed");
        }

        let created = now_ms();
        let id = new_task_id();
        let mut task = AgentTaskRecord::new(&params, id, created);
        self.store.save(&task)?;

        // Create associated WorkTask for autonomy tasks to make them visible
        if matches!(params.kind, TaskKind::AutonomyExplore | TaskKind::AutonomyExecute | TaskKind::WorkTaskExecute) {
            if let Some(emp_id) = &params.executor_id {
                let _ = crate::autonomy_task::create_work_task_for_autonomy(
                    self.store.workspace(),
                    emp_id,
                    &params.kind,
                    &task.id,
                );
            }
        }

        let started = now_ms();
        task.mark_running(started);
        self.store.save(&task)?;
        task_runtime_handle().track(&task.id);

        match self.execute_code_chat_inner(tools, &params, &mut task) {
            Ok((instance, result)) => {
                self.finalize_task_success(tools, &params, &mut task, &instance, &result)?;
                Ok((task, instance, result))
            }
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("task_cancelled") {
                    if let Some(executor_id) = task.executor_id.as_deref() {
                        let _ = self.try_drain_queued_reruns(tools, executor_id);
                    }
                    return Err(err);
                }
                self.finalize_task_error(tools, &params, &mut task, msg)?;
                Err(err)
            }
        }
    }

    pub fn rerun_code_chat(
        &self,
        tools: &ToolManager,
        task_id: &str,
        params: CodeAgentTaskParams,
    ) -> anyhow::Result<(AgentTaskRecord, ToolInstance, ToolExecutionResult)> {
        // If shop is closed, queue for rerun instead of failing.
        if self.shop_is_closed() {
            return self.queue_rerun(task_id).map(|t| {
                // Return a placeholder result - the task will be executed when shop reopens
                let instance = ToolInstance {
                    id: "queued".to_string(),
                    kind: crate::tools::model::ToolKind::ClaudeCode,
                    name: "Queued".to_string(),
                    enabled: true,
                    version: 1,
                    config: serde_json::json!({}),
                };
                let result = ToolExecutionResult {
                    output: "Task queued - shop is closed".to_string(),
                    exit_code: 0,
                    usage: crate::tools::driver::ToolUsage {
                        model: "queued".to_string(),
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    },
                };
                (t, instance, result)
            });
        }

        let mut task = self.store.load(task_id)?;
        if !can_rerun_task(&task) {
            anyhow::bail!("task_cannot_rerun");
        }

        if can_stop_task(&task) {
            task_runtime_handle().request_stop(task_id);
        }

        let started = now_ms();
        task.reset_for_rerun(started);
        self.store.save(&task)?;
        task_runtime_handle().track(&task.id);

        match self.execute_code_chat_inner(tools, &params, &mut task) {
            Ok((instance, result)) => {
                self.finalize_task_success(tools, &params, &mut task, &instance, &result)?;
                Ok((task, instance, result))
            }
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("task_cancelled") {
                    if let Some(executor_id) = task.executor_id.as_deref() {
                        let _ = self.try_drain_queued_reruns(tools, executor_id);
                    }
                    return Err(err);
                }
                self.finalize_task_error(tools, &params, &mut task, msg)?;
                Err(err)
            }
        }
    }

    pub async fn run_code_chat_streaming_events(
        &self,
        tools: &ToolManager,
        params: CodeAgentTaskParams,
        event_tx: tokio::sync::mpsc::Sender<ChatStreamEvent>,
    ) -> anyhow::Result<(AgentTaskRecord, ToolInstance, ToolExecutionResult, Vec<ChatStreamEvent>)>
    {
        let task = self.prepare_streaming_code_chat(&params)?;
        if self.shop_is_closed() {
            emit_streaming_failure(&event_tx, "shop_is_closed").await;
            return Err(streaming_failure(&task.id, "shop_is_closed".to_string()));
        }
        self.execute_prepared_streaming_code_chat(tools, &task.id, params, event_tx)
            .await
    }

    /// Creates and persists a streaming code-agent task before execution starts so
    /// callers can surface it in the UI immediately.
    pub fn prepare_streaming_code_chat(
        &self,
        params: &CodeAgentTaskParams,
    ) -> anyhow::Result<AgentTaskRecord> {
        let created = now_ms();
        let id = new_task_id();
        let mut task = AgentTaskRecord::new(params, id, created);
        if self.shop_is_closed() {
            task.status = TaskStatus::Pending;
            self.store.save(&task)?;
            return Ok(task);
        }

        self.store.save(&task)?;
        self.attach_work_task_for_streaming(params, &task.id)?;
        task.mark_running(now_ms());
        self.store.save(&task)?;
        Ok(task)
    }

    /// Executes a task previously created by [`Self::prepare_streaming_code_chat`].
    pub async fn execute_prepared_streaming_code_chat(
        &self,
        tools: &ToolManager,
        task_id: &str,
        params: CodeAgentTaskParams,
        event_tx: tokio::sync::mpsc::Sender<ChatStreamEvent>,
    ) -> anyhow::Result<(AgentTaskRecord, ToolInstance, ToolExecutionResult, Vec<ChatStreamEvent>)> {
        let mut task = self.store.load(task_id)?;
        if self.shop_is_closed() {
            emit_streaming_failure(&event_tx, "shop_is_closed").await;
            return Err(streaming_failure(task_id, "shop_is_closed".to_string()));
        }

        let lang = crate::agent_locale::resolve_lang_for_workspace(self.store.workspace());
        let messages =
            crate::agent_locale::ensure_language_system_message(params.messages.clone(), lang);

        const MAX_RETRIES: u32 = 2;
        const RETRY_DELAY_MS: u64 = 500;
        let mut last_err = None;

        for attempt in 0..=MAX_RETRIES {
            match tools
                .execute_code_chat_streaming_events(&params.workdir, &messages, event_tx.clone())
                .await
            {
                Ok((instance, result, events)) => {
                    self.finalize_streaming_success(&params.kind, &mut task, &instance, &result)?;
                    return Ok((task, instance, result, events));
                }
                Err(err) => {
                    let msg = err.to_string();
                    if msg.contains("no_enabled_coding_tool") && attempt < MAX_RETRIES {
                        tracing::warn!(attempt = attempt, "no coding tool available (streaming), retrying...");
                        tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                        last_err = Some(err);
                        continue;
                    }
                    last_err = Some(err);
                    break;
                }
            }
        }

        let err_msg = last_err
            .as_ref()
            .map(|e| e.to_string())
            .unwrap_or_else(|| "execute_failed".to_string());
        self.finalize_streaming_error(&params.kind, &mut task, err_msg.clone())?;
        emit_streaming_failure(&event_tx, &err_msg).await;
        Err(streaming_failure(&task.id, err_msg))
    }

    fn attach_work_task_for_streaming(
        &self,
        params: &CodeAgentTaskParams,
        task_id: &str,
    ) -> anyhow::Result<()> {
        if matches!(
            params.kind,
            TaskKind::AutonomyExplore | TaskKind::AutonomyExecute | TaskKind::WorkTaskExecute
        ) {
            if let Some(emp_id) = &params.executor_id {
                let _ = crate::autonomy_task::create_work_task_for_autonomy(
                    self.store.workspace(),
                    emp_id,
                    &params.kind,
                    task_id,
                );
            }
        }
        Ok(())
    }

    /// Resets an existing task so it can be rerun, returning the now-running
    /// record. Splitting this out from execution lets the HTTP handler respond
    /// immediately with an accurate (running) status while the agent runs off the
    /// request path via [`Self::execute_rerun_streaming_events`].
    pub fn prepare_rerun(&self, task_id: &str) -> anyhow::Result<AgentTaskRecord> {
        // If shop is closed, queue for rerun instead of failing.
        if self.shop_is_closed() {
            return self.queue_rerun(task_id);
        }

        let mut task = self.store.load(task_id)?;
        if !can_rerun_task(&task) {
            anyhow::bail!("task_cannot_rerun");
        }

        if can_stop_task(&task) {
            task_runtime_handle().request_stop(task_id);
        }

        let started = now_ms();
        task.reset_for_rerun(started);
        self.store.save(&task)?;
        task_runtime_handle().track(&task.id);
        Ok(task)
    }

    /// Executes an already-prepared (running) rerun while streaming its events,
    /// mirroring the live output into the caller-provided channel. Unlike
    /// [`Self::rerun_code_chat`] this uses the structured streaming driver so the
    /// employee chat panel can render the process in real time.
    ///
    /// Callers must invoke [`Self::prepare_rerun`] first.
    pub async fn execute_rerun_streaming_events(
        &self,
        tools: &ToolManager,
        task_id: &str,
        params: CodeAgentTaskParams,
        event_tx: tokio::sync::mpsc::Sender<ChatStreamEvent>,
    ) -> anyhow::Result<AgentTaskRecord> {
        let mut task = self.store.load(task_id)?;

        let lang = crate::agent_locale::resolve_lang_for_workspace(self.store.workspace());
        let messages =
            crate::agent_locale::ensure_language_system_message(params.messages.clone(), lang);

        let result = tools
            .execute_code_chat_streaming_events(&params.workdir, &messages, event_tx.clone())
            .await;
        task_runtime_handle().unregister(&task.id);
        match result {
            Ok((instance, exec_result, _events)) => {
                self.finalize_streaming_success(&params.kind, &mut task, &instance, &exec_result)?;
                Ok(task)
            }
            Err(err) => {
                let err_msg = err.to_string();
                self.finalize_streaming_error(&params.kind, &mut task, err_msg.clone())?;
                emit_streaming_failure(&event_tx, &err_msg).await;
                Err(streaming_failure(&task.id, err_msg))
            }
        }
    }

    fn finalize_streaming_success(
        &self,
        kind: &TaskKind,
        task: &mut AgentTaskRecord,
        instance: &ToolInstance,
        result: &ToolExecutionResult,
    ) -> anyhow::Result<()> {
        task.complete_with_result(instance, result, now_ms());
        self.store.save(task)?;
        if !result.output.is_empty() {
            let _ = self.store.save_output(&task.id, &result.output);
        }
        if matches!(
            kind,
            TaskKind::AutonomyExplore | TaskKind::AutonomyExecute | TaskKind::WorkTaskExecute
        ) {
            let _ = crate::autonomy_task::sync_work_task_status(
                self.store.workspace(),
                &task.id,
                &task.status,
            );
        }
        self.notify_autonomy_if_needed(kind, task);
        Ok(())
    }

    fn finalize_streaming_error(
        &self,
        kind: &TaskKind,
        task: &mut AgentTaskRecord,
        error: String,
    ) -> anyhow::Result<()> {
        task.fail(error, now_ms());
        self.store.save(task)?;
        if matches!(
            kind,
            TaskKind::AutonomyExplore | TaskKind::AutonomyExecute | TaskKind::WorkTaskExecute
        ) {
            let _ = crate::autonomy_task::sync_work_task_status(
                self.store.workspace(),
                &task.id,
                &task.status,
            );
        }
        self.notify_autonomy_if_needed(kind, task);
        Ok(())
    }

    fn notify_autonomy_if_needed(&self, kind: &TaskKind, task: &AgentTaskRecord) {
        if matches!(kind, TaskKind::AutonomyExplore) {
            return;
        }
        if let Some(employee_id) = task.executor_id.as_deref() {
            let _ = mark_employee_for_autonomy(self.store.workspace(), employee_id);
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

pub fn can_rerun_task(task: &AgentTaskRecord) -> bool {
    matches!(
        task.status,
        TaskStatus::Pending
            | TaskStatus::Running
            | TaskStatus::Completed
            | TaskStatus::Failed
            | TaskStatus::Cancelled
            | TaskStatus::QueuedRerun
    )
}

pub fn should_queue_rerun_instead(source: &AgentTaskRecord, tasks: &[AgentTaskRecord]) -> bool {
    if !source.is_terminal() && !source.is_queued_for_rerun() {
        return false;
    }
    let Some(executor_id) = source.executor_id.as_deref() else {
        return false;
    };
    is_employee_busy_excluding(tasks, executor_id, Some(&source.id))
}

pub fn can_stop_task(task: &AgentTaskRecord) -> bool {
    matches!(task.status, TaskStatus::Pending | TaskStatus::Running)
}

pub fn build_rerun_params(task: &AgentTaskRecord) -> CodeAgentTaskParams {
    use crate::tools::driver::ToolChatMessage;
    use std::path::PathBuf;

    CodeAgentTaskParams {
        kind: task.kind,
        content: task.content.clone(),
        workdir: PathBuf::from(&task.workdir),
        messages: vec![ToolChatMessage {
            role: "user".into(),
            content: task.content.clone(),
        }],
        executor_id: task.executor_id.clone(),
        parent_task_id: task.parent_task_id.clone(),
        context: task.context.clone(),
    }
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
    fn can_rerun_includes_running_and_pending_tasks() {
        let params = CodeAgentTaskParams {
            kind: TaskKind::RequirementAgent,
            content: "hello".into(),
            workdir: std::path::PathBuf::from("/tmp"),
            messages: vec![],
            executor_id: Some("emp-1".into()),
            parent_task_id: None,
            context: serde_json::json!({}),
        };
        for status in [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ] {
            let mut task = AgentTaskRecord::new(&params, "t1".into(), 1);
            task.status = status;
            assert!(can_rerun_task(&task));
        }
    }

    #[test]
    fn build_rerun_params_preserves_parent_task_id() {
        let params = CodeAgentTaskParams {
            kind: TaskKind::AutonomyExplore,
            content: "explore workspace".into(),
            workdir: std::path::PathBuf::from("/tmp/ws"),
            messages: vec![],
            executor_id: Some("alice".into()),
            parent_task_id: Some("parent-1".into()),
            context: serde_json::json!({ "employee_id": "alice" }),
        };
        let source = AgentTaskRecord::new(&params, "task_old".into(), 100);
        let rerun = build_rerun_params(&source);
        assert_eq!(rerun.kind, TaskKind::AutonomyExplore);
        assert_eq!(rerun.content, "explore workspace");
        assert_eq!(rerun.workdir, std::path::PathBuf::from("/tmp/ws"));
        assert_eq!(rerun.executor_id.as_deref(), Some("alice"));
        assert_eq!(rerun.parent_task_id.as_deref(), Some("parent-1"));
        assert_eq!(rerun.messages.len(), 1);
        assert_eq!(rerun.messages[0].content, "explore workspace");
    }

    #[test]
    fn rerun_code_chat_reuses_task_id_and_marks_running_before_execute() {
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
        let mut task = AgentTaskRecord::new(&params, "task_rerun_1".into(), 100);
        task.status = TaskStatus::Failed;
        task.error = Some("tool_exit_code_1".into());
        task.ended_at_ms = Some(200);
        TaskStore::new(&workspace).save(&task).unwrap();

        let err = runner
            .rerun_code_chat(&tools, "task_rerun_1", build_rerun_params(&task))
            .unwrap_err();
        assert!(err.to_string().contains("no_enabled_coding_tool"));

        let tasks = TaskStore::new(&workspace).list().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "task_rerun_1");
        assert_eq!(tasks[0].status, TaskStatus::Failed);
        assert!(tasks[0].started_at_ms.is_some());
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn prepare_streaming_code_chat_persists_running_task() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let runner = TaskRunner::new(&workspace);
        let params = CodeAgentTaskParams {
            kind: TaskKind::WorkTaskExecute,
            content: "Start application".into(),
            workdir: workspace.clone(),
            messages: vec![],
            executor_id: Some("emp-ops".into()),
            parent_task_id: None,
            context: serde_json::json!({ "requirement_id": "demo" }),
        };

        let prepared = runner.prepare_streaming_code_chat(&params).unwrap();
        assert_eq!(prepared.status, TaskStatus::Running);
        assert_eq!(prepared.executor_id.as_deref(), Some("emp-ops"));

        let loaded = TaskStore::new(&workspace).load(&prepared.id).unwrap();
        assert_eq!(loaded.status, TaskStatus::Running);
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn queue_rerun_marks_completed_task_as_queued() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let runner = TaskRunner::new(&workspace);
        let params = CodeAgentTaskParams {
            kind: TaskKind::RequirementAgent,
            content: "hello".into(),
            workdir: workspace.clone(),
            messages: vec![],
            executor_id: Some("emp-1".into()),
            parent_task_id: None,
            context: serde_json::json!({}),
        };
        let mut task = AgentTaskRecord::new(&params, "task_queue_1".into(), 100);
        task.status = TaskStatus::Completed;
        TaskStore::new(&workspace).save(&task).unwrap();

        let queued = runner.queue_rerun("task_queue_1").unwrap();
        assert_eq!(queued.status, TaskStatus::QueuedRerun);
        assert!(queued.is_queued_for_rerun());

        let loaded = TaskStore::new(&workspace).list().unwrap();
        assert_eq!(loaded[0].status, TaskStatus::QueuedRerun);
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn should_queue_rerun_when_other_task_is_active() {
        let params = CodeAgentTaskParams {
            kind: TaskKind::RequirementAgent,
            content: "hello".into(),
            workdir: std::path::PathBuf::from("/tmp"),
            messages: vec![],
            executor_id: Some("emp-1".into()),
            parent_task_id: None,
            context: serde_json::json!({}),
        };
        let mut completed = AgentTaskRecord::new(&params, "done".into(), 1);
        completed.status = TaskStatus::Completed;
        let mut running = AgentTaskRecord::new(&params, "run".into(), 2);
        running.status = TaskStatus::Running;
        assert!(should_queue_rerun_instead(
            &completed,
            &[completed.clone(), running]
        ));
    }

    #[test]
    fn stop_task_marks_pending_task_cancelled() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let runner = TaskRunner::new(&workspace);
        let params = CodeAgentTaskParams {
            kind: TaskKind::RequirementAgent,
            content: "hello".into(),
            workdir: workspace.clone(),
            messages: vec![],
            executor_id: Some("emp-1".into()),
            parent_task_id: None,
            context: serde_json::json!({}),
        };
        let task = AgentTaskRecord::new(&params, "task_stop_1".into(), 100);
        TaskStore::new(&workspace).save(&task).unwrap();

        let stopped = runner.stop_task("task_stop_1").unwrap();
        assert_eq!(stopped.status, TaskStatus::Cancelled);
        assert_eq!(stopped.error.as_deref(), Some("task_stopped_by_user"));

        let err = runner.stop_task("task_stop_1").unwrap_err().to_string();
        assert_eq!(err, "task_cannot_stop");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn stop_all_tasks_for_executor_stops_only_stoppable_tasks() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let runner = TaskRunner::new(&workspace);
        let store = TaskStore::new(&workspace);
        let params = |id: &str, status: TaskStatus| {
            let mut task = AgentTaskRecord::new(
                &CodeAgentTaskParams {
                    kind: TaskKind::RequirementAgent,
                    content: id.into(),
                    workdir: workspace.clone(),
                    messages: vec![],
                    executor_id: Some("emp-1".into()),
                    parent_task_id: None,
                    context: serde_json::json!({}),
                },
                id.into(),
                100,
            );
            task.status = status;
            task
        };
        store.save(&params("pending-1", TaskStatus::Pending)).unwrap();
        store.save(&params("running-1", TaskStatus::Running)).unwrap();
        store.save(&params("done-1", TaskStatus::Completed)).unwrap();
        store
            .save(&{
                let mut other = params("other-pending", TaskStatus::Pending);
                other.executor_id = Some("emp-2".into());
                other
            })
            .unwrap();

        let stopped = runner.stop_all_tasks_for_executor("emp-1").unwrap();
        assert_eq!(stopped.len(), 2);
        assert!(stopped.iter().all(|t| t.status == TaskStatus::Cancelled));

        let all = store.list().unwrap();
        let done = all.iter().find(|t| t.id == "done-1").unwrap();
        assert_eq!(done.status, TaskStatus::Completed);
        let other = all.iter().find(|t| t.id == "other-pending").unwrap();
        assert_eq!(other.status, TaskStatus::Pending);
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn can_stop_only_active_tasks() {
        let params = CodeAgentTaskParams {
            kind: TaskKind::RequirementAgent,
            content: "hello".into(),
            workdir: std::path::PathBuf::from("/tmp"),
            messages: vec![],
            executor_id: Some("emp-1".into()),
            parent_task_id: None,
            context: serde_json::json!({}),
        };
        for status in [TaskStatus::Pending, TaskStatus::Running] {
            let mut task = AgentTaskRecord::new(&params, "t1".into(), 1);
            task.status = status;
            assert!(can_stop_task(&task));
        }
        for status in [
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ] {
            let mut task = AgentTaskRecord::new(&params, "t1".into(), 1);
            task.status = status;
            assert!(!can_stop_task(&task));
        }
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
