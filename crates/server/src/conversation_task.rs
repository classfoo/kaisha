use crate::employee_chat::{
    conversation_path, load_conversation, new_message_id, save_conversation, schedule_conversation_save,
    ConversationFile, StoredMessage, STREAM_PERSIST_INTERVAL,
};
use crate::tasks::AgentTaskRecord;
use crate::tools::driver::ChatStreamEvent;
use std::future::Future;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Runs a streaming code-agent task while mirroring its live output into the
/// employee's conversation file so the chat panel shows the real-time process.
///
/// This is the shared bridge used by every flow that should surface a
/// code-agent run in the employee chat list (explore, task rerun, development
/// task execution, ...). It:
///
/// 1. appends a `task_process` message to `shachiku/{employee}/conversation.json`,
/// 2. persists the incremental `stream_events` produced by the agent (the
///    conversation watch SSE endpoint relays these to the frontend in real time),
/// 3. finalizes the message status (`completed`/`failed`) once `run` resolves.
///
/// `run` receives the event sender and must drive the streaming code-agent,
/// returning the resulting [`AgentTaskRecord`]. The original task result is
/// returned unchanged so callers can perform any follow-up bookkeeping.
pub async fn run_with_conversation<F, Fut>(
    workspace: &Path,
    employee_id: &str,
    run: F,
) -> anyhow::Result<AgentTaskRecord>
where
    F: FnOnce(tokio::sync::mpsc::Sender<ChatStreamEvent>) -> Fut,
    Fut: Future<Output = anyhow::Result<AgentTaskRecord>>,
{
    let conv_path = conversation_path(workspace, employee_id);
    let mut conv = load_conversation(&conv_path).unwrap_or_else(|_| ConversationFile {
        version: 1,
        messages: vec![],
    });
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let task_process_idx = conv.messages.len();
    conv.messages.push(StoredMessage {
        id: new_message_id("msg_task_process"),
        role: "task_process".to_string(),
        content: "".to_string(),
        created_at_ms: now_ms,
        sender_name: None,
        sender_avatar_url: None,
        task_id: None,
        task_status: Some("running".to_string()),
        stream_events: None,
        result_meta: None,
    });
    if let Err(e) = save_conversation(&conv_path, &conv) {
        tracing::warn!(error = %e, "failed to save conversation before streaming task");
    }

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<ChatStreamEvent>(32);

    // Collect streamed events and persist them incrementally so the conversation
    // watch endpoint can relay each step to the frontend as it happens.
    let conv_path_clone = conv_path.clone();
    let collect_handle = tokio::spawn(async move {
        let mut collected_events: Vec<serde_json::Value> = Vec::new();
        let mut events_since_save = 0;
        let save_interval = STREAM_PERSIST_INTERVAL;

        let mut conv = load_conversation(&conv_path_clone).unwrap_or_else(|_| ConversationFile {
            version: 1,
            messages: vec![],
        });

        while let Some(event) = event_rx.recv().await {
            if let Some(val) = serde_json::to_value(&event).ok() {
                collected_events.push(val);
                events_since_save += 1;

                if task_process_idx < conv.messages.len() {
                    conv.messages[task_process_idx].stream_events = Some(collected_events.clone());
                }

                if events_since_save >= save_interval {
                    schedule_conversation_save(conv_path_clone.clone(), conv.clone());
                    events_since_save = 0;
                }
            }
        }

        schedule_conversation_save(conv_path_clone, conv);
    });

    let task_result = run(event_tx).await;

    // Ensure the collector has flushed every event before we finalize.
    let _ = collect_handle.await;

    let mut conv = load_conversation(&conv_path).unwrap_or_else(|_| ConversationFile {
        version: 1,
        messages: vec![],
    });
    match &task_result {
        Ok(task) => {
            if task_process_idx < conv.messages.len() {
                conv.messages[task_process_idx].task_id = Some(task.id.clone());
                conv.messages[task_process_idx].task_status = Some("completed".to_string());
                if conv.messages[task_process_idx].content.is_empty() {
                    conv.messages[task_process_idx].content = task.content.clone();
                }
            }
        }
        Err(e) => {
            if task_process_idx < conv.messages.len() {
                conv.messages[task_process_idx].task_status = Some("failed".to_string());
                conv.messages[task_process_idx].content = e.to_string();
            }
        }
    }
    let _ = save_conversation(&conv_path, &conv);

    task_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{CodeAgentTaskParams, TaskKind};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-conversation-task-{unique}"))
    }

    fn sample_task(content: &str) -> AgentTaskRecord {
        AgentTaskRecord::new(
            &CodeAgentTaskParams {
                kind: TaskKind::WorkTaskExecute,
                content: content.to_string(),
                workdir: std::path::PathBuf::from("/tmp"),
                messages: vec![],
                executor_id: Some("alice".into()),
                parent_task_id: None,
                context: serde_json::json!({}),
            },
            "task_generated_1".into(),
            100,
        )
    }

    #[tokio::test]
    async fn streams_events_into_employee_conversation() {
        let workspace = temp_workspace();
        std::fs::create_dir_all(&workspace).unwrap();

        let result = run_with_conversation(&workspace, "alice", |tx| async move {
            tx.send(ChatStreamEvent::AssistantText {
                text: "step one".into(),
            })
            .await
            .unwrap();
            tx.send(ChatStreamEvent::Thinking {
                text: "pondering".into(),
            })
            .await
            .unwrap();
            Ok(sample_task("final reply"))
        })
        .await;

        assert!(result.is_ok());

        let conv = load_conversation(&conversation_path(&workspace, "alice")).unwrap();
        assert_eq!(conv.messages.len(), 1);
        let msg = &conv.messages[0];
        assert_eq!(msg.role, "task_process");
        assert_eq!(msg.task_status.as_deref(), Some("completed"));
        assert_eq!(msg.task_id.as_deref(), Some("task_generated_1"));
        assert_eq!(msg.content, "final reply");
        let events = msg.stream_events.as_ref().expect("stream events persisted");
        assert_eq!(events.len(), 2);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn marks_conversation_failed_when_task_errors() {
        let workspace = temp_workspace();
        std::fs::create_dir_all(&workspace).unwrap();

        let result = run_with_conversation(&workspace, "bob", |tx| async move {
            drop(tx);
            Err(anyhow::anyhow!("no_enabled_coding_tool"))
        })
        .await;

        assert!(result.is_err());

        let conv = load_conversation(&conversation_path(&workspace, "bob")).unwrap();
        assert_eq!(conv.messages.len(), 1);
        let msg = &conv.messages[0];
        assert_eq!(msg.role, "task_process");
        assert_eq!(msg.task_status.as_deref(), Some("failed"));
        assert_eq!(msg.content, "no_enabled_coding_tool");

        let _ = std::fs::remove_dir_all(&workspace);
    }
}
