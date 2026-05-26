use super::model::AgentTaskRecord;
use super::store::TaskStore;
use crate::tools::model::ToolKind;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AgentTaskExecutionInfo {
    pub tool_instance_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_kind: Option<ToolKind>,
    pub model: Option<String>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentTaskDetail {
    pub task: AgentTaskRecord,
    pub output: Option<String>,
    pub execution: AgentTaskExecutionInfo,
}

pub fn build_task_detail(store: &TaskStore, task: AgentTaskRecord) -> AgentTaskDetail {
    let duration_ms = match (task.started_at_ms, task.ended_at_ms) {
        (Some(start), Some(end)) if end >= start => Some(end - start),
        _ => None,
    };
    let output = store
        .load_output(&task.id)
        .ok()
        .flatten()
        .or_else(|| task.output_preview.clone());
    let execution = AgentTaskExecutionInfo {
        tool_instance_id: task.tool_instance_id.clone(),
        tool_name: task.tool_name.clone(),
        tool_kind: task.tool_kind.clone(),
        model: task.model.clone(),
        prompt_tokens: task.prompt_tokens,
        completion_tokens: task.completion_tokens,
        total_tokens: task.total_tokens,
        exit_code: task.exit_code,
        error: task.error.clone(),
        duration_ms,
    };
    AgentTaskDetail {
        task,
        output,
        execution,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::model::{CodeAgentTaskParams, TaskKind, TaskStatus};
    use crate::tasks::store::TaskStore;
    use crate::tools::model::ToolKind as ModelToolKind;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-task-detail-{unique}"))
    }

    #[test]
    fn build_task_detail_prefers_full_output_file() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let store = TaskStore::new(&workspace);
        let mut task = AgentTaskRecord::new(
            &CodeAgentTaskParams {
                kind: TaskKind::RequirementAgent,
                content: "do work".into(),
                workdir: workspace.clone(),
                messages: vec![],
                executor_id: Some("alice".into()),
                parent_task_id: None,
                context: serde_json::json!({}),
            },
            "task_detail_1".into(),
            100,
        );
        task.status = TaskStatus::Completed;
        task.started_at_ms = Some(200);
        task.ended_at_ms = Some(900);
        task.output_preview = Some("preview".into());
        store.save(&task).unwrap();
        store.save_output("task_detail_1", "full agent output").unwrap();

        let detail = build_task_detail(&store, task);
        assert_eq!(detail.output.as_deref(), Some("full agent output"));
        assert_eq!(detail.execution.duration_ms, Some(700));

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn build_task_detail_falls_back_to_preview() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let store = TaskStore::new(&workspace);
        let mut task = AgentTaskRecord::new(
            &CodeAgentTaskParams {
                kind: TaskKind::RequirementAgent,
                content: "do work".into(),
                workdir: workspace.clone(),
                messages: vec![],
                executor_id: None,
                parent_task_id: None,
                context: serde_json::json!({}),
            },
            "task_detail_2".into(),
            100,
        );
        task.tool_kind = Some(ModelToolKind::ClaudeCode);
        task.model = Some("claude-test".into());
        task.output_preview = Some("preview only".into());
        store.save(&task).unwrap();

        let detail = build_task_detail(&store, store.load("task_detail_2").unwrap());
        assert_eq!(detail.output.as_deref(), Some("preview only"));
        assert_eq!(detail.execution.tool_kind, Some(ModelToolKind::ClaudeCode));

        let _ = fs::remove_dir_all(&workspace);
    }
}
