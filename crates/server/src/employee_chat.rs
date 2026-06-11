use crate::{
    employee::{employee_root, normalize_employee_id},
    employee_requirement_agent::{
        build_requirement_agent_messages, prior_conversation_context, requirement_agent_workdir,
    },
    i18n,
    intent::context::IntentContext,
    requirement::list_requirement_summaries,
    requirement_review::run_requirement_review,
    tasks::{
        task_content_from_user_input, CodeAgentTaskParams, TaskKind, TaskRunner,
    },
    tools::manager::ToolManager,
    AppState,
};
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_stream::wrappers::ReceiverStream;

/// Number of streamed events to buffer before flushing the conversation file.
/// Kept at 1 so the file reflects code-agent output incrementally, which the
/// conversation watch endpoint relays to the frontend in real time.
pub(crate) const STREAM_PERSIST_INTERVAL: usize = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConversationFile {
    pub(crate) version: u32,
    pub(crate) messages: Vec<StoredMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredMessage {
    pub(crate) id: String,
    pub(crate) role: String,
    pub(crate) content: String,
    pub(crate) created_at_ms: u64,
    #[serde(default)]
    pub(crate) sender_name: Option<String>,
    #[serde(default)]
    pub(crate) sender_avatar_url: Option<String>,
    #[serde(default)]
    pub(crate) task_id: Option<String>,
    #[serde(default)]
    pub(crate) task_status: Option<String>,
    #[serde(default)]
    pub(crate) stream_events: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub(crate) result_meta: Option<PostMessageResultMeta>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WireMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_events: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_meta: Option<PostMessageResultMeta>,
}

impl From<&StoredMessage> for WireMessage {
    fn from(m: &StoredMessage) -> Self {
        Self {
            id: m.id.clone(),
            role: m.role.clone(),
            content: m.content.clone(),
            created_at_ms: m.created_at_ms,
            sender_name: m.sender_name.clone(),
            sender_avatar_url: m.sender_avatar_url.clone(),
            task_id: m.task_id.clone(),
            task_status: m.task_status.clone(),
            stream_events: m.stream_events.clone(),
            result_meta: m.result_meta.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct MessagesResponse {
    pub messages: Vec<WireMessage>,
}

#[derive(Deserialize)]
pub struct PostMessageBody {
    pub content: String,
    #[serde(default)]
    pub sender_name: Option<String>,
    #[serde(default)]
    pub sender_avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostMessageResultMeta {
    pub exit_code: i32,
    pub tool_instance_id: String,
    pub tool_kind: crate::tools::model::ToolKind,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_preview: Option<String>,
}

#[derive(Serialize)]
pub struct PostMessageResponse {
    pub messages: Vec<WireMessage>,
    pub last_result: PostMessageResultMeta,
}

fn workspace_dir(state: &AppState) -> Option<PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

pub(crate) fn conversation_path(workspace: &Path, employee_id: &str) -> PathBuf {
    employee_root(workspace).join(employee_id).join("conversation.json")
}

fn employee_profile_path(workspace: &Path, employee_id: &str) -> PathBuf {
    employee_root(workspace).join(employee_id).join("profile.json")
}

pub(crate) fn load_conversation(path: &Path) -> anyhow::Result<ConversationFile> {
    if !path.exists() {
        return Ok(ConversationFile {
            version: 1,
            messages: vec![],
        });
    }
    // Retry on transient read errors (e.g. file being written concurrently).
    let mut last_error = None;
    for attempt in 0..3 {
        match fs::read_to_string(path) {
            Ok(raw) => match serde_json::from_str(&raw) {
                Ok(conv) => return Ok(conv),
                Err(e) => last_error = Some(e.into()),
            },
            Err(e) => last_error = Some(e.into()),
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("conversation_load_failed")))
}

pub(crate) fn save_conversation(path: &Path, file: &ConversationFile) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(file)?)?;
    Ok(())
}

pub(crate) fn new_message_id(prefix: &str) -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{prefix}_{ms}")
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let t = s.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    })
}

#[allow(dead_code)]
fn format_review_assistant_reply(
    review: &crate::requirement_review::RequirementReviewWire,
) -> String {
    let conclusion = review
        .conclusion
        .as_ref()
        .map(|c| match c {
            crate::requirement_review::ReviewConclusion::Adopt => "adopt",
            crate::requirement_review::ReviewConclusion::Supplement => "supplement",
        })
        .unwrap_or("pending");
    let summary = review
        .summary
        .as_deref()
        .unwrap_or("(no summary file)");
    format!(
        "Requirement review completed for `{}`.\n\n**Conclusion:** {conclusion}\n\n## Summary\n\n{summary}",
        review.requirement_id
    )
}

/// Routes the user intent through the intent router and returns the result.
/// This is the main entry point for intent-based chat responses.
fn process_intent_via_router(
    tools: &ToolManager,
    workspace: &Path,
    employee_id: &str,
    user_input: &str,
    conv_messages: &[StoredMessage],
    known_req_ids: &[String],
    known_emp_ids: &[String],
) -> Result<(crate::tools::model::ToolInstance, crate::tools::driver::ToolExecutionResult, Option<String>, Option<String>), String> {
    let ctx = IntentContext {
        workspace,
        employee_id,
        known_requirement_ids: known_req_ids.to_vec(),
        known_employee_ids: known_emp_ids.to_vec(),
    };

    let router = crate::intent::handlers::create_default_router();

    // Detect the intent
    let detection = router.detect(user_input, &ctx);

    // Route to the appropriate handler or fall back to default
    match detection {
        Some(det) => {
            if let Some(handler) = router.handlers.get(&det.intent_type) {
                handler.handle(&det, tools, workspace, employee_id, user_input, &conv_messages_to_prior(conv_messages))
                    .map(|result| {
                        let instance = tools.pick_enabled_chat_driver()
                            .map(|(inst, _)| inst)
                            .unwrap_or_else(|| {
                                // For terminal intents that don't need a coding tool
                                crate::tools::model::ToolInstance {
                                    id: "intent".to_string(),
                                    kind: crate::tools::model::ToolKind::ClaudeCode,
                                    name: "Intent Handler".to_string(),
                                    enabled: true,
                                    version: 1,
                                    config: serde_json::json!({}),
                                }
                            });
                        (
                            instance,
                            result.execution_result.unwrap_or_else(|| crate::tools::driver::ToolExecutionResult {
                                output: result.output.clone(),
                                exit_code: 0,
                                usage: crate::tools::driver::ToolUsage {
                                    model: "intent-router".to_string(),
                                    prompt_tokens: 0,
                                    completion_tokens: 0,
                                    total_tokens: 0,
                                },
                            }),
                            result.task_id,
                            result.output_preview,
                        )
                    })
            } else {
                // No handler registered for this intent, fall back to default
                run_requirement_agent_turn_internal(tools, workspace, employee_id, user_input, conv_messages)
            }
        }
        None => {
            // No intent detected, fall back to default
            run_requirement_agent_turn_internal(tools, workspace, employee_id, user_input, conv_messages)
        }
    }
}

fn conv_messages_to_prior(messages: &[StoredMessage]) -> Vec<(String, String)> {
    let prior_rows: Vec<(String, String, u64)> = messages
        .iter()
        .map(|m| (m.role.clone(), m.content.clone(), m.created_at_ms))
        .collect();
    prior_conversation_context(&prior_rows)
}

#[allow(dead_code)]
fn run_review_turn(
    tools: &ToolManager,
    workspace: &Path,
    user_input: &str,
    _conv_messages: &[(String, String)],
) -> Result<(crate::tools::model::ToolInstance, crate::tools::driver::ToolExecutionResult, Option<String>, Option<String>), String> {
    let summaries = list_requirement_summaries(workspace).map_err(|e| e.to_string())?;
    let ids: Vec<String> = summaries.iter().map(|s| s.id.clone()).collect();

    // Extract requirement ID from input
    let req_id = crate::employee_intent_router::extract_requirement_id(user_input, &ids)
        .ok_or_else(|| "review_requirement_unspecified".to_string())?;

    let review = run_requirement_review(workspace, tools, &req_id).map_err(|e| e.to_string())?;
    let output = format_review_assistant_reply(&review);
    let instance = tools
        .pick_enabled_chat_driver()
        .map(|(inst, _)| inst)
        .ok_or_else(|| "chat_tool_missing".to_string())?;
    Ok((
        instance,
        crate::tools::driver::ToolExecutionResult {
            output,
            exit_code: 0,
            usage: crate::tools::driver::ToolUsage {
                model: "requirement-review".to_string(),
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        },
        None,
        None,
    ))
}

fn run_requirement_agent_turn_internal(
    tools: &ToolManager,
    workspace: &Path,
    employee_id: &str,
    user_input: &str,
    conv_messages: &[StoredMessage],
) -> Result<(crate::tools::model::ToolInstance, crate::tools::driver::ToolExecutionResult, Option<String>, Option<String>), String> {
    let workdir = requirement_agent_workdir(workspace).map_err(|e| e.to_string())?;
    let prior = conv_messages_to_prior(conv_messages);
    let tool_messages =
        build_requirement_agent_messages(workspace, user_input, &prior).map_err(|e| e.to_string())?;
    let runner = TaskRunner::new(workspace);
    runner
        .run_code_chat(
            tools,
            CodeAgentTaskParams {
                kind: TaskKind::RequirementAgent,
                content: task_content_from_user_input(user_input),
                workdir: workdir.clone(),
                messages: tool_messages,
                executor_id: Some(employee_id.to_string()),
                parent_task_id: None,
                context: serde_json::json!({ "employee_id": employee_id }),
            },
        )
        .map(|(task, instance, result)| (instance, result, Some(task.id), task.output_preview.clone()))
        .map_err(|e| {
            let msg = e.root_cause().to_string();
            if msg == "no_enabled_coding_tool" {
                "chat_tool_missing".to_string()
            } else {
                tracing::warn!(error = %e, "requirement agent execution failed");
                msg
            }
        })
}

/// Persistence context for streaming events. When present, events are written
/// to the conversation file in addition to being forwarded via SSE.
struct StreamPersistence {
    conv_path: PathBuf,
    conv: ConversationFile,
    task_process_msg_idx: usize,
    save_interval: usize,
}

/// Runs the streaming code agent for SSE responses.
/// If `persistence` is provided, events are also saved to the conversation file.
async fn run_streaming_agent(
    workspace: &Path,
    tools: &ToolManager,
    employee_id: &str,
    user_input: &str,
    conv_messages: &[StoredMessage],
    sse_tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
    persistence: Option<StreamPersistence>,
) -> Result<(crate::tools::model::ToolInstance, crate::tools::driver::ToolExecutionResult, Option<String>, Option<String>), String> {
    let workdir = requirement_agent_workdir(workspace).map_err(|e| e.to_string())?;
    let prior = conv_messages_to_prior(conv_messages);
    let tool_messages =
        build_requirement_agent_messages(workspace, user_input, &prior).map_err(|e| e.to_string())?;

    let (event_tx, event_rx) =
        tokio::sync::mpsc::channel::<crate::tools::driver::ChatStreamEvent>(64);

    let forward: tokio::task::JoinHandle<Vec<serde_json::Value>> = if let Some(p) = persistence {
        tokio::spawn(forward_sse_events_with_persistence(
            event_rx,
            sse_tx.clone(),
            p.conv_path,
            p.conv,
            p.task_process_msg_idx,
            p.save_interval,
        ))
    } else {
        tokio::spawn(async move {
            forward_sse_events(event_rx, sse_tx.clone()).await;
            vec![]
        })
    };

    let runner = TaskRunner::new(workspace);
    let result = runner
        .run_code_chat_streaming_events(
            tools,
            CodeAgentTaskParams {
                kind: TaskKind::RequirementAgent,
                content: task_content_from_user_input(user_input),
                workdir: workdir.clone(),
                messages: tool_messages,
                executor_id: Some(employee_id.to_string()),
                parent_task_id: None,
                context: serde_json::json!({ "employee_id": employee_id }),
            },
            event_tx,
        )
        .await
        .map_err(|e| {
            let msg = e.root_cause().to_string();
            if msg == "no_enabled_coding_tool" {
                "chat_tool_missing".to_string()
            } else {
                tracing::warn!(error = %e, "requirement agent execution failed (stream)");
                msg
            }
        });
    match result {
        Ok((task, instance, tool_result, _events)) => {
            let _collected = forward.await;
            Ok((instance, tool_result, Some(task.id), task.output_preview.clone()))
        }
        Err(raw) => {
            forward.abort();
            Err(raw)
        }
    }
}

fn map_process_error(err: String) -> &'static str {
    if err == "no_enabled_coding_tool" || err == "chat_tool_missing" {
        return "chat_tool_missing";
    }
    if err == "shop_is_closed" {
        return "shop_is_closed";
    }
    match err.as_str() {
        "employee_not_found" => "employee_not_found",
        "chat_prompt_empty" => "chat_prompt_empty",
        "chat_tool_missing" => "chat_tool_missing",
        "shop_is_closed" => "shop_is_closed",
        _ => "chat_tool_run_failed",
    }
}

fn process_post_message(
    tools: ToolManager,
    workspace: PathBuf,
    employee_id: String,
    content: String,
    sender_name: Option<String>,
    sender_avatar_url: Option<String>,
) -> Result<PostMessageResponse, String> {
    let profile = employee_profile_path(&workspace, &employee_id);
    if !profile.exists() {
        return Err("employee_not_found".to_string());
    }
    let conv_path = conversation_path(&workspace, &employee_id);
    let mut conv = load_conversation(&conv_path).map_err(|e| e.to_string())?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        return Err("chat_prompt_empty".to_string());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let user_content = trimmed.clone();
    conv.messages.push(StoredMessage {
        id: new_message_id("msg_user"),
        role: "user".to_string(),
        content: user_content,
        created_at_ms: now,
        sender_name,
        sender_avatar_url,
        task_id: None,
        task_status: None,
        stream_events: None,
        result_meta: None,
    });

    let summaries = list_requirement_summaries(&workspace).map_err(|e| e.to_string())?;
    let employees = crate::employee::list_employee_records(&workspace).map_err(|e| e.to_string())?;
    let req_ids: Vec<String> = summaries.iter().map(|s| s.id.clone()).collect();
    let emp_ids: Vec<String> = employees.iter().map(|e| e.id.clone()).collect();

    let (instance, exec_result, task_id, output_preview) = process_intent_via_router(
        &tools,
        &workspace,
        &employee_id,
        &trimmed,
        &conv.messages,
        &req_ids,
        &emp_ids,
    )?;

    let assistant_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    conv.messages.push(StoredMessage {
        id: new_message_id("msg_assistant"),
        role: "assistant".to_string(),
        content: exec_result.output.clone(),
        created_at_ms: assistant_now,
        sender_name: None,
        sender_avatar_url: None,
        task_id: None,
        task_status: None,
        stream_events: None,
        result_meta: None,
    });

    save_conversation(&conv_path, &conv).map_err(|e| e.to_string())?;

    let meta = PostMessageResultMeta {
        exit_code: exec_result.exit_code,
        tool_instance_id: instance.id.clone(),
        tool_kind: instance.kind.clone(),
        model: exec_result.usage.model.clone(),
        prompt_tokens: exec_result.usage.prompt_tokens,
        completion_tokens: exec_result.usage.completion_tokens,
        total_tokens: exec_result.usage.total_tokens,
        task_id,
        output_preview,
    };

    Ok(PostMessageResponse {
        messages: conv.messages.iter().map(WireMessage::from).collect(),
        last_result: meta,
    })
}

fn status_for_process_key(key: &str) -> axum::http::StatusCode {
    match key {
        "employee_not_found" => axum::http::StatusCode::NOT_FOUND,
        "chat_prompt_empty" => axum::http::StatusCode::BAD_REQUEST,
        "chat_tool_missing" => axum::http::StatusCode::CONFLICT,
        "shop_is_closed" => axum::http::StatusCode::SERVICE_UNAVAILABLE,
        _ => axum::http::StatusCode::BAD_GATEWAY,
    }
}

pub async fn get_messages(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(employee_id): AxumPath<String>,
) -> Result<Json<MessagesResponse>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_dir(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let employee_id = normalize_employee_id(&employee_id).map_err(|err| {
        let key = err.to_string();
        (
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, key.as_str()),
        )
    })?;
    let profile = employee_profile_path(&workspace, &employee_id);
    if !profile.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "employee_not_found"),
        ));
    }
    let conv_path = conversation_path(&workspace, &employee_id);
    // If conversation fails to load, return empty messages instead of an error
    // to avoid breaking the chat UI. The user can still send new messages.
    let conv = load_conversation(&conv_path).unwrap_or_else(|_| {
        tracing::warn!(path = ?conv_path, "failed to load conversation, returning empty");
        ConversationFile {
            version: 1,
            messages: vec![],
        }
    });
    Ok(Json(MessagesResponse {
        messages: conv.messages.iter().map(WireMessage::from).collect(),
    }))
}

pub async fn post_message(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(employee_id): AxumPath<String>,
    Json(body): Json<PostMessageBody>,
) -> Result<Json<PostMessageResponse>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_dir(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let employee_id = normalize_employee_id(&employee_id).map_err(|err| {
        let key = err.to_string();
        (
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, key.as_str()),
        )
    })?;
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workspace_path = workspace;
    let PostMessageBody {
        content,
        sender_name,
        sender_avatar_url,
    } = body;
    let sender_name = normalize_optional_text(sender_name);
    let sender_avatar_url = normalize_optional_text(sender_avatar_url);
    let res = tokio::task::spawn_blocking(move || {
        process_post_message(
            tools,
            workspace_path,
            employee_id,
            content,
            sender_name,
            sender_avatar_url,
        )
    })
        .await
        .map_err(|_e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                i18n::msg(&headers, "chat_blocking_task_failed"),
            )
        })?
        .map_err(|raw| {
            let key = map_process_error(raw.clone());
            (
                status_for_process_key(key),
                if key == "chat_tool_run_failed" && raw != "chat_tool_run_failed" {
                    format!("{}: {}", i18n::msg(&headers, key), raw)
                } else {
                    i18n::msg(&headers, key)
                },
            )
        })?;
    Ok(Json(res))
}

async fn forward_sse_events(
    mut event_rx: tokio::sync::mpsc::Receiver<crate::tools::driver::ChatStreamEvent>,
    tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) {
    use crate::tools::driver::ChatStreamEvent;
    while let Some(event) = event_rx.recv().await {
        // Pick a stable SSE event name per variant; payload is the serialized event.
        let event_name = match &event {
            ChatStreamEvent::Start { .. } => "start",
            ChatStreamEvent::AssistantText { .. } => "delta",
            ChatStreamEvent::Thinking { .. } => "thinking",
            ChatStreamEvent::ToolUse { .. } => "tool_use",
            ChatStreamEvent::ToolResult { .. } => "tool_result",
            ChatStreamEvent::Result { .. } => "result",
            ChatStreamEvent::Raw { .. } => "delta",
        };
        // For backwards compatibility, the `delta` event still carries `{ "text": "..." }`.
        let payload = match &event {
            ChatStreamEvent::AssistantText { text } | ChatStreamEvent::Raw { text } => {
                serde_json::to_string(&serde_json::json!({ "text": text }))
            }
            _ => serde_json::to_string(&event),
        };
        let Ok(data) = payload else { continue };
        if tx
            .send(Ok(Event::default().event(event_name).data(data)))
            .await
            .is_err()
        {
            break;
        }
    }
}

/// Variant of forward_sse_events that also collects events into the conversation file.
/// Events are appended to the task_process message's stream_events array.
/// Conversation is saved periodically (every N events) and at the end.
async fn forward_sse_events_with_persistence(
    mut event_rx: tokio::sync::mpsc::Receiver<crate::tools::driver::ChatStreamEvent>,
    tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
    conv_path: PathBuf,
    mut conv: ConversationFile,
    task_process_msg_idx: usize,
    save_interval: usize,
) -> Vec<serde_json::Value> {
    use crate::tools::driver::ChatStreamEvent;
    let mut collected_events: Vec<serde_json::Value> = Vec::new();
    let mut events_since_save = 0;

    let save_conv = |conv: &ConversationFile| {
        if let Err(e) = save_conversation(&conv_path, conv) {
            tracing::warn!(error = %e, "failed to save conversation during streaming");
        }
    };

    while let Some(event) = event_rx.recv().await {
        // Serialize event as JSON value for storage
        let event_value = match &event {
            ChatStreamEvent::Start { .. } => {
                serde_json::to_value(&event).ok()
            }
            ChatStreamEvent::AssistantText { .. } => {
                // For assistant_text, also emit a 'delta' event for SSE compatibility
                serde_json::to_value(&event).ok()
            }
            ChatStreamEvent::Thinking { .. } => serde_json::to_value(&event).ok(),
            ChatStreamEvent::ToolUse { .. } => serde_json::to_value(&event).ok(),
            ChatStreamEvent::ToolResult { .. } => serde_json::to_value(&event).ok(),
            ChatStreamEvent::Result { .. } => serde_json::to_value(&event).ok(),
            ChatStreamEvent::Raw { .. } => serde_json::to_value(&event).ok(),
        };

        // Pick a stable SSE event name per variant
        let event_name = match &event {
            ChatStreamEvent::Start { .. } => "start",
            ChatStreamEvent::AssistantText { .. } => "delta",
            ChatStreamEvent::Thinking { .. } => "thinking",
            ChatStreamEvent::ToolUse { .. } => "tool_use",
            ChatStreamEvent::ToolResult { .. } => "tool_result",
            ChatStreamEvent::Result { .. } => "result",
            ChatStreamEvent::Raw { .. } => "delta",
        };
        let payload = match &event {
            ChatStreamEvent::AssistantText { text } | ChatStreamEvent::Raw { text } => {
                serde_json::to_string(&serde_json::json!({ "text": text }))
            }
            _ => serde_json::to_string(&event),
        };
        let Ok(data) = payload else { continue };

        // Forward to SSE client
        if tx
            .send(Ok(Event::default().event(event_name).data(data)))
            .await
            .is_err()
        {
            break;
        }

        // Store event for persistence
        if let Some(val) = event_value {
            collected_events.push(val);
            events_since_save += 1;

            // Update the task_process message with collected events
            if task_process_msg_idx < conv.messages.len() {
                conv.messages[task_process_msg_idx].stream_events = Some(collected_events.clone());
            }

            // Periodic save
            if events_since_save >= save_interval {
                save_conv(&conv);
                events_since_save = 0;
            }
        }
    }

    // Final save
    save_conv(&conv);
    collected_events
}

async fn run_stream_turn_inner(
    tools: ToolManager,
    workspace: PathBuf,
    employee_id: String,
    content: String,
    sender_name: Option<String>,
    sender_avatar_url: Option<String>,
    sse_tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) -> Result<(), String> {
    let profile = employee_profile_path(&workspace, &employee_id);
    if !profile.exists() {
        return Err("employee_not_found".to_string());
    }
    let conv_path = conversation_path(&workspace, &employee_id);
    let mut conv = load_conversation(&conv_path).map_err(|e| e.to_string())?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        return Err("chat_prompt_empty".to_string());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let user_content = trimmed.clone();
    conv.messages.push(StoredMessage {
        id: new_message_id("msg_user"),
        role: "user".to_string(),
        content: user_content,
        created_at_ms: now,
        sender_name,
        sender_avatar_url,
        task_id: None,
        task_status: None,
        stream_events: None,
        result_meta: None,
    });
    save_conversation(&conv_path, &conv).map_err(|e| e.to_string())?;

    let summaries = list_requirement_summaries(&workspace).map_err(|e| e.to_string())?;
    let employees = crate::employee::list_employee_records(&workspace).map_err(|e| e.to_string())?;
    let req_ids: Vec<String> = summaries.iter().map(|s| s.id.clone()).collect();
    let emp_ids: Vec<String> = employees.iter().map(|e| e.id.clone()).collect();

    let ctx = IntentContext {
        workspace: &workspace,
        employee_id: &employee_id,
        known_requirement_ids: req_ids,
        known_employee_ids: emp_ids,
    };

    let router = crate::intent::handlers::create_default_router();
    let detection = router.detect(&trimmed, &ctx);

    let (instance, tool_result, task_id, output_preview) = if detection.is_some() {
        // Route through the intent router
        let prior = conv_messages_to_prior(&conv.messages);
        let result = router.route_and_handle(
            &trimmed,
            &ctx,
            &tools,
            &workspace,
            &employee_id,
            &prior,
        )
        .map_err(|e| {
            tracing::warn!(error = %e, "intent handler failed (stream)");
            e
        })?;
        let payload = serde_json::to_string(&serde_json::json!({ "text": result.output }))
            .map_err(|e| e.to_string())?;
        let _ = sse_tx
            .send(Ok(Event::default().event("delta").data(payload)))
            .await;
        let inst = tools.pick_enabled_chat_driver()
            .map(|(inst, _)| inst)
            .unwrap_or_else(|| crate::tools::model::ToolInstance {
                id: "intent".to_string(),
                kind: crate::tools::model::ToolKind::ClaudeCode,
                name: "Intent Handler".to_string(),
                enabled: true,
                version: 1,
                config: serde_json::json!({}),
            });
        (
            inst,
            result.execution_result.unwrap_or_else(|| crate::tools::driver::ToolExecutionResult {
                output: result.output.clone(),
                exit_code: 0,
                usage: crate::tools::driver::ToolUsage {
                    model: "intent-router".to_string(),
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
            }),
            result.task_id,
            result.output_preview,
        )
    } else {
        // No intent detected, fall back to streaming agent.
        // Create a task_process message to track the streaming execution.
        let task_process_idx = conv.messages.len();
        conv.messages.push(StoredMessage {
            id: new_message_id("msg_task_process"),
            role: "task_process".to_string(),
            content: "".to_string(),
            created_at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            sender_name: None,
            sender_avatar_url: None,
            task_id: None,
            task_status: Some("running".to_string()),
            stream_events: None,
            result_meta: None,
        });
        // Save immediately so the task_process message is persisted
        save_conversation(&conv_path, &conv).map_err(|e| e.to_string())?;

        let persistence = StreamPersistence {
            conv_path: conv_path.clone(),
            conv: conv.clone(),
            task_process_msg_idx: task_process_idx,
            save_interval: STREAM_PERSIST_INTERVAL,
        };
        let result = run_streaming_agent(
            &workspace,
            &tools,
            &employee_id,
            &trimmed,
            &conv.messages,
            sse_tx.clone(),
            Some(persistence),
        )
        .await?;

        // Update task_process message with final status and task_id
        if task_process_idx < conv.messages.len() {
            conv.messages[task_process_idx].task_id = result.2.clone();
            conv.messages[task_process_idx].task_status = Some("completed".to_string());
            conv.messages[task_process_idx].content = result.1.output.clone();
        }

        // We need conv to be updated, so clone it from the result
        // Actually, we need to reload conv since forward_sse_events_with_persistence modified it
        // Re-read it from disk (the persistence task saved it)
        conv = load_conversation(&conv_path).map_err(|e| e.to_string())?;
        // Ensure the task_process is updated
        if task_process_idx < conv.messages.len() {
            conv.messages[task_process_idx].task_status = Some("completed".to_string());
            conv.messages[task_process_idx].task_id = result.2.clone();
            if conv.messages[task_process_idx].content.is_empty() {
                conv.messages[task_process_idx].content = result.1.output.clone();
            }
        }

        result
    };

    let assistant_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    conv.messages.push(StoredMessage {
        id: new_message_id("msg_assistant"),
        role: "assistant".to_string(),
        content: tool_result.output.clone(),
        created_at_ms: assistant_now,
        sender_name: None,
        sender_avatar_url: None,
        task_id: None,
        task_status: None,
        stream_events: None,
        result_meta: None,
    });
    save_conversation(&conv_path, &conv).map_err(|e| e.to_string())?;

    let meta = PostMessageResultMeta {
        exit_code: tool_result.exit_code,
        tool_instance_id: instance.id.clone(),
        tool_kind: instance.kind.clone(),
        model: tool_result.usage.model.clone(),
        prompt_tokens: tool_result.usage.prompt_tokens,
        completion_tokens: tool_result.usage.completion_tokens,
        total_tokens: tool_result.usage.total_tokens,
        task_id,
        output_preview,
    };
    let resp = PostMessageResponse {
        messages: conv.messages.iter().map(WireMessage::from).collect(),
        last_result: meta,
    };
    let data = serde_json::to_string(&resp).map_err(|e| e.to_string())?;
    let _ = sse_tx
        .send(Ok(Event::default().event("done").data(data)))
        .await;
    Ok(())
}

pub async fn post_message_stream(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(employee_id): AxumPath<String>,
    Json(body): Json<PostMessageBody>,
) -> Result<Sse<ReceiverStream<Result<Event, Infallible>>>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_dir(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let employee_id = normalize_employee_id(&employee_id).map_err(|err| {
        let key = err.to_string();
        (
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, key.as_str()),
        )
    })?;
    let profile = employee_profile_path(&workspace, &employee_id);
    if !profile.exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "employee_not_found"),
        ));
    }

    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workspace_path = workspace;
    let PostMessageBody {
        content,
        sender_name,
        sender_avatar_url,
    } = body;
    let sender_name = normalize_optional_text(sender_name);
    let sender_avatar_url = normalize_optional_text(sender_avatar_url);

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);

    let err_tool_missing = i18n::msg(&headers, "chat_tool_missing");
    let err_prompt_empty = i18n::msg(&headers, "chat_prompt_empty");
    let err_employee = i18n::msg(&headers, "employee_not_found");
    let err_tool_run = i18n::msg(&headers, "chat_tool_run_failed");

    tokio::spawn(async move {
        let sse_tx = tx.clone();
        let r = run_stream_turn_inner(
            tools,
            workspace_path,
            employee_id,
            content,
            sender_name,
            sender_avatar_url,
            sse_tx.clone(),
        )
        .await;
        if let Err(raw) = r {
            let key = map_process_error(raw.clone());
            let msg = match key {
                "employee_not_found" => err_employee.clone(),
                "chat_prompt_empty" => err_prompt_empty.clone(),
                "chat_tool_missing" => err_tool_missing.clone(),
                _ => {
                    if key == "chat_tool_run_failed" && raw != "chat_tool_run_failed" {
                        format!("{err_tool_run}: {raw}")
                    } else {
                        err_tool_run.clone()
                    }
                }
            };
            let payload = serde_json::json!({ "message": msg }).to_string();
            let _ = sse_tx
                .send(Ok(Event::default().event("error").data(payload)))
                .await;
        }
    });

    Ok(
        Sse::new(ReceiverStream::new(rx))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(20))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_message_preserves_sender_fields() {
        let m = StoredMessage {
            id: "1".into(),
            role: "user".into(),
            content: "hi".into(),
            created_at_ms: 9,
            sender_name: Some("Alice".into()),
            sender_avatar_url: Some("https://example.com/a.png".into()),
            task_id: None,
            task_status: None,
            stream_events: None,
            result_meta: None,
        };
        let w = WireMessage::from(&m);
        assert_eq!(w.sender_name.as_deref(), Some("Alice"));
        assert_eq!(w.sender_avatar_url.as_deref(), Some("https://example.com/a.png"));
    }

    #[test]
    fn deserialize_stored_message_omits_sender_by_default() {
        let raw = r#"{"id":"a","role":"user","content":"x","created_at_ms":0}"#;
        let m: StoredMessage = serde_json::from_str(raw).unwrap();
        assert!(m.sender_name.is_none());
        assert!(m.sender_avatar_url.is_none());
    }

    #[test]
    fn normalize_optional_text_trims_and_drops_empty() {
        assert_eq!(normalize_optional_text(Some("  hi  ".into())), Some("hi".into()));
        assert_eq!(normalize_optional_text(Some("   ".into())), None);
        assert_eq!(normalize_optional_text(None), None);
    }

    #[test]
    fn map_process_error_maps_known_codes() {
        assert_eq!(map_process_error("chat_tool_missing".into()), "chat_tool_missing");
        assert_eq!(map_process_error("employee_not_found".into()), "employee_not_found");
        assert_eq!(map_process_error("something else".into()), "chat_tool_run_failed");
    }

    #[test]
    fn format_review_assistant_reply_includes_conclusion_and_summary() {
        let review = crate::requirement_review::RequirementReviewWire {
            requirement_id: "req-1".into(),
            status: crate::requirement_review::ReviewStatus::Completed,
            started_at_ms: 0,
            completed_at_ms: Some(1),
            conclusion: Some(crate::requirement_review::ReviewConclusion::Adopt),
            participants: vec![],
            opinions: vec![],
            summary: Some("All good".into()),
            passed_count: 1,
            failed_count: 0,
            pending_count: 0,
            undecided_count: 0,
            abandoned_count: 0,
            overall_passed: true,
        };
        let text = format_review_assistant_reply(&review);
        assert!(text.contains("req-1"));
        assert!(text.contains("adopt"));
        assert!(text.contains("All good"));
    }
}
