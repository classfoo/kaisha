use crate::employee::normalize_employee_id;
use crate::employee_chat::{conversation_path, load_conversation, save_conversation, StoredMessage, WireMessage};
use crate::i18n;
use crate::tasks::runtime::task_runtime_handle;
use crate::tasks::TaskStore;
use crate::AppState;
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
};
use std::{
    collections::HashMap,
    convert::Infallible,
    hash::{Hash, Hasher},
    path::Path,
    time::{Duration, UNIX_EPOCH},
};
use tokio_stream::wrappers::ReceiverStream;

/// How often the watch loop polls the conversation file for changes.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Result of diffing a conversation snapshot against the previously seen state.
pub(crate) struct ConversationDiff {
    /// Messages that are new or whose content changed since the last snapshot.
    pub(crate) changed: Vec<StoredMessage>,
    /// The full snapshot map (message id -> content hash) after this diff.
    pub(crate) snapshot: HashMap<String, u64>,
}

/// Stable content hash of a stored message. Serializing the whole message means
/// growth of the streamed `stream_events` array (incremental code-agent output)
/// is detected as a change, which is what drives incremental delivery.
fn message_hash(msg: &StoredMessage) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    match serde_json::to_string(msg) {
        Ok(serialized) => serialized.hash(&mut hasher),
        Err(_) => msg.id.hash(&mut hasher),
    }
    hasher.finish()
}

/// Diffs the current conversation messages against a previously seen snapshot.
/// Returns only the messages that are new or changed, plus the updated snapshot.
pub(crate) fn diff_conversation(
    prev: &HashMap<String, u64>,
    messages: &[StoredMessage],
) -> ConversationDiff {
    let mut changed = Vec::new();
    let mut snapshot = HashMap::with_capacity(messages.len());
    for msg in messages {
        let hash = message_hash(msg);
        snapshot.insert(msg.id.clone(), hash);
        match prev.get(&msg.id) {
            Some(prev_hash) if *prev_hash == hash => {}
            _ => changed.push(msg.clone()),
        }
    }
    ConversationDiff { changed, snapshot }
}

/// Cheap change signature for the conversation file: (mtime_ms, len). When this
/// is unchanged we skip parsing the file entirely.
fn file_signature(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let len = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Some((mtime, len))
}

/// SSE endpoint that watches an employee's conversation file and streams
/// incremental message updates to the frontend in real time.
///
/// Emits `update` events carrying `{ "messages": [WireMessage, ...] }` whenever
/// the conversation file changes (including while a code-agent task streams its
/// output into the file). Because the file is the single source of truth, this
/// reflects output from any producer (interactive chat, autonomy execute/explore,
/// background task runs) without coupling to the request that started the task.
pub async fn conversation_stream_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(employee_id): AxumPath<String>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    let workspace = state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone();
    let Some(workspace) = workspace else {
        let msg = i18n::msg(&headers, "workspace_not_configured");
        let _ = tx
            .send(Ok(Event::default()
                .event("error")
                .data(serde_json::json!({ "message": msg }).to_string())))
            .await;
        return sse_response(rx);
    };

    let employee_id = match normalize_employee_id(&employee_id) {
        Ok(id) => id,
        Err(_) => {
            let msg = i18n::msg(&headers, "employee_not_found");
            let _ = tx
                .send(Ok(Event::default()
                    .event("error")
                    .data(serde_json::json!({ "message": msg }).to_string())))
                .await;
            return sse_response(rx);
        }
    };

    let conv_path = conversation_path(&workspace, &employee_id);

    tokio::spawn(async move {
        let mut snapshot: HashMap<String, u64> = HashMap::new();
        let mut last_sig: Option<(u64, u64)> = None;
        let runtime = task_runtime_handle();
        let task_store = TaskStore::new(&workspace);
        // Send an initial `ready` marker so the client knows the watch is live.
        if tx
            .send(Ok(Event::default().event("ready").data("{}")))
            .await
            .is_err()
        {
            return;
        }

        loop {
            if tx.is_closed() {
                break;
            }
            let sig = file_signature(&conv_path);
            if sig != last_sig {
                last_sig = sig;
                if let Ok(mut conv) = load_conversation(&conv_path) {
                    // Detect crashed tasks in real-time
                    let mut conv_changed = false;
                    for msg in conv.messages.iter_mut() {
                        if msg.task_status.as_deref() == Some("running") {
                            if let Some(ref task_id) = msg.task_id {
                                if !runtime.is_tracked(task_id) {
                                    tracing::warn!(task_id = %task_id, "stream detected crashed task");
                                    if let Ok(mut task) = task_store.load(task_id) {
                                        task.fail("process_crashed".to_string(), crate::tasks::now_ms());
                                        let _ = task_store.save(&task);
                                    }
                                    msg.task_status = Some("failed".to_string());
                                    conv_changed = true;
                                }
                            }
                        }
                    }
                    if conv_changed {
                        let _ = save_conversation(&conv_path, &conv);
                    }

                    let diff = diff_conversation(&snapshot, &conv.messages);
                    snapshot = diff.snapshot;
                    if !diff.changed.is_empty() {
                        let wire: Vec<WireMessage> =
                            diff.changed.iter().map(WireMessage::from).collect();
                        let payload = serde_json::json!({ "messages": wire }).to_string();
                        if tx
                            .send(Ok(Event::default().event("update").data(payload)))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });

    sse_response(rx)
}

fn sse_response(
    rx: tokio::sync::mpsc::Receiver<Result<Event, Infallible>>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: &str, content: &str) -> StoredMessage {
        StoredMessage {
            id: id.to_string(),
            role: "task_process".to_string(),
            content: content.to_string(),
            created_at_ms: 0,
            sender_name: None,
            sender_avatar_url: None,
            task_id: None,
            task_status: None,
            stream_events: None,
            result_meta: None,
        }
    }

    #[test]
    fn diff_detects_new_changed_and_skips_unchanged() {
        let prev = HashMap::new();
        let m1 = msg("a", "hello");
        let d1 = diff_conversation(&prev, std::slice::from_ref(&m1));
        assert_eq!(d1.changed.len(), 1, "first sighting yields the message");

        let d2 = diff_conversation(&d1.snapshot, std::slice::from_ref(&m1));
        assert_eq!(d2.changed.len(), 0, "unchanged message is not re-sent");

        let m1b = msg("a", "hello world");
        let d3 = diff_conversation(&d2.snapshot, std::slice::from_ref(&m1b));
        assert_eq!(d3.changed.len(), 1, "changed content is detected");
        assert_eq!(d3.changed[0].content, "hello world");

        let d4 = diff_conversation(&d3.snapshot, &[m1b.clone(), msg("b", "second")]);
        assert_eq!(d4.changed.len(), 1, "only the newly appended message is sent");
        assert_eq!(d4.changed[0].id, "b");
    }

    #[test]
    fn diff_detects_growing_stream_events() {
        let prev = HashMap::new();
        let mut running = msg("p", "");
        running.task_status = Some("running".to_string());
        running.stream_events = Some(vec![serde_json::json!({ "type": "assistant_text", "text": "a" })]);
        let d1 = diff_conversation(&prev, std::slice::from_ref(&running));
        assert_eq!(d1.changed.len(), 1);

        running.stream_events = Some(vec![
            serde_json::json!({ "type": "assistant_text", "text": "a" }),
            serde_json::json!({ "type": "assistant_text", "text": "b" }),
        ]);
        let d2 = diff_conversation(&d1.snapshot, std::slice::from_ref(&running));
        assert_eq!(
            d2.changed.len(),
            1,
            "appended stream events count as an incremental change"
        );
    }
}
