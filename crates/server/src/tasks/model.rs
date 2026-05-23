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
}

pub const OUTPUT_PREVIEW_MAX: usize = 2000;

pub fn truncate_preview(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max).collect();
    format!("{truncated}…")
}
