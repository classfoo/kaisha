use crate::tools::model::ToolKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    QueuedRerun,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    EmployeeHire,
    RequirementAgent,
    ReviewOpinion,
    ReviewRevision,
    ReviewSummary,
    ReviewPipeline,
    AutonomyExplore,
    AutonomyExecute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskRecord {
    pub id: String,
    pub kind: TaskKind,
    pub content: String,
    pub workdir: String,
    pub tool_instance_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_kind: Option<ToolKind>,
    pub executor_id: Option<String>,
    pub status: TaskStatus,
    pub created_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub output_preview: Option<String>,
    pub model: Option<String>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    #[serde(default)]
    pub context: Value,
}

#[derive(Debug, Clone)]
pub struct CodeAgentTaskParams {
    pub kind: TaskKind,
    pub content: String,
    pub workdir: std::path::PathBuf,
    pub messages: Vec<crate::tools::driver::ToolChatMessage>,
    pub executor_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub context: Value,
}

impl AgentTaskRecord {
    pub fn new(params: &CodeAgentTaskParams, id: String, created_at_ms: u64) -> Self {
        Self {
            id,
            kind: params.kind,
            content: params.content.clone(),
            workdir: params.workdir.to_string_lossy().to_string(),
            tool_instance_id: None,
            tool_name: None,
            tool_kind: None,
            executor_id: params.executor_id.clone(),
            status: TaskStatus::Pending,
            created_at_ms,
            started_at_ms: None,
            ended_at_ms: None,
            exit_code: None,
            error: None,
            output_preview: None,
            model: None,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            parent_task_id: params.parent_task_id.clone(),
            context: params.context.clone(),
        }
    }

    pub fn mark_running(&mut self, started_at_ms: u64) {
        self.status = TaskStatus::Running;
        self.started_at_ms = Some(started_at_ms);
    }

    pub fn reset_for_rerun(&mut self, started_at_ms: u64) {
        self.status = TaskStatus::Running;
        self.started_at_ms = Some(started_at_ms);
        self.ended_at_ms = None;
        self.exit_code = None;
        self.error = None;
        self.output_preview = None;
        self.tool_instance_id = None;
        self.tool_name = None;
        self.tool_kind = None;
        self.model = None;
        self.prompt_tokens = 0;
        self.completion_tokens = 0;
        self.total_tokens = 0;
        self.clear_queued_rerun_marker();
    }

    pub fn complete_with_result(
        &mut self,
        instance: &crate::tools::model::ToolInstance,
        result: &crate::tools::driver::ToolExecutionResult,
        ended_at_ms: u64,
    ) {
        self.status = if result.exit_code == 0 {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        self.tool_instance_id = Some(instance.id.clone());
        self.tool_name = Some(instance.name.clone());
        self.tool_kind = Some(instance.kind.clone());
        self.ended_at_ms = Some(ended_at_ms);
        self.exit_code = Some(result.exit_code);
        self.output_preview = Some(truncate_preview(&result.output, OUTPUT_PREVIEW_MAX));
        self.model = Some(result.usage.model.clone());
        self.prompt_tokens = result.usage.prompt_tokens;
        self.completion_tokens = result.usage.completion_tokens;
        self.total_tokens = result.usage.total_tokens;
        if result.exit_code != 0 {
            self.error = Some(format!("tool_exit_code_{}", result.exit_code));
        }
    }

    pub fn fail(&mut self, error: String, ended_at_ms: u64) {
        self.status = TaskStatus::Failed;
        self.ended_at_ms = Some(ended_at_ms);
        self.error = Some(error);
    }

    pub fn cancel(&mut self, ended_at_ms: u64) {
        self.status = TaskStatus::Cancelled;
        self.ended_at_ms = Some(ended_at_ms);
        self.error = Some("task_stopped_by_user".into());
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    pub fn is_queued_for_rerun(&self) -> bool {
        self.status == TaskStatus::QueuedRerun
    }

    pub fn queued_rerun_at_ms(&self) -> u64 {
        self.context
            .get("queued_rerun_at_ms")
            .and_then(|value| value.as_u64())
            .unwrap_or(self.created_at_ms)
    }

    pub fn mark_queued_rerun(&mut self, at_ms: u64) {
        self.status = TaskStatus::QueuedRerun;
        self.started_at_ms = None;
        self.ended_at_ms = None;
        self.exit_code = None;
        self.error = None;
        self.output_preview = None;
        self.tool_instance_id = None;
        self.tool_name = None;
        self.tool_kind = None;
        self.model = None;
        self.prompt_tokens = 0;
        self.completion_tokens = 0;
        self.total_tokens = 0;
        if let serde_json::Value::Object(map) = &mut self.context {
            map.insert("queued_rerun_at_ms".into(), serde_json::json!(at_ms));
        }
    }

    pub fn clear_queued_rerun_marker(&mut self) {
        if let serde_json::Value::Object(map) = &mut self.context {
            map.remove("queued_rerun_at_ms");
        }
    }
}

pub const OUTPUT_PREVIEW_MAX: usize = 2000;

pub fn truncate_preview(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{
        driver::{ToolExecutionResult, ToolUsage},
        model::{ToolInstance, ToolKind},
    };

    fn sample_params() -> CodeAgentTaskParams {
        CodeAgentTaskParams {
            kind: TaskKind::RequirementAgent,
            content: "do work".into(),
            workdir: std::path::PathBuf::from("/tmp/ws/requirements"),
            messages: vec![],
            executor_id: Some("alice".into()),
            parent_task_id: Some("parent-1".into()),
            context: serde_json::json!({ "requirement_id": "r1" }),
        }
    }

    fn sample_instance() -> ToolInstance {
        ToolInstance {
            id: "tool_1".into(),
            kind: ToolKind::ClaudeCode,
            name: "Claude".into(),
            enabled: true,
            version: 1,
            config: serde_json::json!({}),
        }
    }

    fn sample_result(exit_code: i32, output: &str) -> ToolExecutionResult {
        ToolExecutionResult {
            output: output.to_string(),
            exit_code,
            usage: ToolUsage {
                model: "test-model".into(),
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            },
        }
    }

    #[test]
    fn new_task_starts_pending_with_fields_from_params() {
        let params = sample_params();
        let task = AgentTaskRecord::new(&params, "task_abc".into(), 1000);
        assert_eq!(task.id, "task_abc");
        assert_eq!(task.kind, TaskKind::RequirementAgent);
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.executor_id.as_deref(), Some("alice"));
        assert_eq!(task.parent_task_id.as_deref(), Some("parent-1"));
        assert_eq!(task.workdir, "/tmp/ws/requirements");
    }

    #[test]
    fn mark_running_sets_status_and_timestamp() {
        let mut task = AgentTaskRecord::new(&sample_params(), "t1".into(), 1);
        task.mark_running(500);
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.started_at_ms, Some(500));
    }

    #[test]
    fn cancel_sets_cancelled_status_and_reason() {
        let mut task = AgentTaskRecord::new(&sample_params(), "t1".into(), 1);
        task.mark_running(2);
        task.cancel(900);
        assert_eq!(task.status, TaskStatus::Cancelled);
        assert_eq!(task.ended_at_ms, Some(900));
        assert_eq!(task.error.as_deref(), Some("task_stopped_by_user"));
    }

    #[test]
    fn mark_queued_rerun_sets_status_and_timestamp() {
        let mut task = AgentTaskRecord::new(&sample_params(), "t1".into(), 1);
        task.complete_with_result(&sample_instance(), &sample_result(0, "done"), 3);
        task.mark_queued_rerun(900);
        assert_eq!(task.status, TaskStatus::QueuedRerun);
        assert!(task.is_queued_for_rerun());
        assert_eq!(task.queued_rerun_at_ms(), 900);
        assert!(task.ended_at_ms.is_none());
    }

    #[test]
    fn reset_for_rerun_clears_execution_fields_and_sets_running() {
        let mut task = AgentTaskRecord::new(&sample_params(), "t1".into(), 1);
        task.mark_running(2);
        task.complete_with_result(&sample_instance(), &sample_result(0, "done"), 3);
        task.reset_for_rerun(900);
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.started_at_ms, Some(900));
        assert!(task.ended_at_ms.is_none());
        assert!(task.exit_code.is_none());
        assert!(task.error.is_none());
        assert!(task.output_preview.is_none());
        assert!(task.tool_instance_id.is_none());
        assert_eq!(task.prompt_tokens, 0);
    }

    #[test]
    fn complete_with_result_records_tool_metadata() {
        let mut task = AgentTaskRecord::new(&sample_params(), "t1".into(), 1);
        let instance = sample_instance();
        let result = sample_result(0, "hello output");
        task.complete_with_result(&instance, &result, 900);
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.tool_instance_id.as_deref(), Some("tool_1"));
        assert_eq!(task.tool_name.as_deref(), Some("Claude"));
        assert_eq!(task.tool_kind, Some(ToolKind::ClaudeCode));
        assert_eq!(task.exit_code, Some(0));
        assert_eq!(task.output_preview.as_deref(), Some("hello output"));
        assert_eq!(task.model.as_deref(), Some("test-model"));
        assert_eq!(task.total_tokens, 30);
        assert!(task.error.is_none());
    }

    #[test]
    fn non_zero_exit_marks_failed_with_error_code() {
        let mut task = AgentTaskRecord::new(&sample_params(), "t1".into(), 1);
        task.complete_with_result(&sample_instance(), &sample_result(2, "err"), 900);
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.error.as_deref(), Some("tool_exit_code_2"));
    }

    #[test]
    fn fail_sets_failed_status_and_error_message() {
        let mut task = AgentTaskRecord::new(&sample_params(), "t1".into(), 1);
        task.fail("no_enabled_coding_tool".into(), 800);
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.error.as_deref(), Some("no_enabled_coding_tool"));
        assert_eq!(task.ended_at_ms, Some(800));
    }

    #[test]
    fn truncate_preview_keeps_short_text_unchanged() {
        assert_eq!(truncate_preview("hello", 10), "hello");
    }

    #[test]
    fn task_record_json_roundtrip() {
        let mut task = AgentTaskRecord::new(&sample_params(), "t1".into(), 1);
        task.mark_running(2);
        task.complete_with_result(&sample_instance(), &sample_result(0, "ok"), 3);
        let json = serde_json::to_string(&task).unwrap();
        let parsed: AgentTaskRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, task.id);
        assert_eq!(parsed.status, TaskStatus::Completed);
        assert_eq!(parsed.tool_kind, Some(ToolKind::ClaudeCode));
    }
}
