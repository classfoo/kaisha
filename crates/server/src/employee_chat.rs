use crate::{
    employee::{employee_root, normalize_employee_id},
    employee_requirement_agent::{
        build_requirement_agent_messages, prior_conversation_context, requirement_agent_workdir,
    },
    i18n,
    requirement::list_requirement_summaries,
    requirement_review::{detect_review_start_intent, run_requirement_review},
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationFile {
    version: u32,
    messages: Vec<StoredMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMessage {
    id: String,
    role: String,
    content: String,
    created_at_ms: u64,
    #[serde(default)]
    sender_name: Option<String>,
    #[serde(default)]
    sender_avatar_url: Option<String>,
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

#[derive(Serialize)]
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

fn conversation_path(workspace: &Path, employee_id: &str) -> PathBuf {
    employee_root(workspace).join(employee_id).join("conversation.json")
}

fn employee_profile_path(workspace: &Path, employee_id: &str) -> PathBuf {
    employee_root(workspace).join(employee_id).join("profile.json")
}

fn load_conversation(path: &Path) -> anyhow::Result<ConversationFile> {
    if !path.exists() {
        return Ok(ConversationFile {
            version: 1,
            messages: vec![],
        });
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn save_conversation(path: &Path, file: &ConversationFile) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(file)?)?;
    Ok(())
}

fn new_message_id(prefix: &str) -> String {
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

fn run_review_turn(
    tools: &ToolManager,
    workspace: &Path,
    user_input: &str,
) -> Result<(crate::tools::model::ToolInstance, crate::tools::driver::ToolExecutionResult), String> {
    let summaries = list_requirement_summaries(workspace).map_err(|e| e.to_string())?;
    let ids: Vec<String> = summaries.iter().map(|s| s.id.clone()).collect();
    let req_id = detect_review_start_intent(user_input, &ids)
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
    ))
}

fn run_requirement_agent_turn(
    tools: &ToolManager,
    workspace: &Path,
    employee_id: &str,
    user_input: &str,
    conv_messages: &[StoredMessage],
) -> Result<(crate::tools::model::ToolInstance, crate::tools::driver::ToolExecutionResult, Option<String>, Option<String>), String> {
    let workdir = requirement_agent_workdir(workspace).map_err(|e| e.to_string())?;
    let prior_rows: Vec<(String, String, u64)> = conv_messages
        .iter()
        .map(|m| (m.role.clone(), m.content.clone(), m.created_at_ms))
        .collect();
    let prior = prior_conversation_context(&prior_rows);
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

fn map_process_error(err: String) -> &'static str {
    if err == "no_enabled_coding_tool" || err == "chat_tool_missing" {
        return "chat_tool_missing";
    }
    match err.as_str() {
        "employee_not_found" => "employee_not_found",
        "chat_prompt_empty" => "chat_prompt_empty",
        "chat_tool_missing" => "chat_tool_missing",
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
    });

    let summaries = list_requirement_summaries(&workspace).map_err(|e| e.to_string())?;
    let ids: Vec<String> = summaries.iter().map(|s| s.id.clone()).collect();
    let (instance, exec_result, task_id, output_preview) = if detect_review_start_intent(&trimmed, &ids).is_some() {
        let (inst, res) = run_review_turn(&tools, &workspace, &trimmed)?;
        (inst, res, None, None)
    } else {
        run_requirement_agent_turn(&tools, &workspace, &employee_id, &trimmed, &conv.messages)?
    };

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
    let conv = load_conversation(&conv_path).map_err(|_e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            i18n::msg(&headers, "chat_conversation_load_failed"),
        )
    })?;
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

async fn forward_sse_deltas(
    mut delta_rx: tokio::sync::mpsc::Receiver<String>,
    tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) {
    while let Some(chunk) = delta_rx.recv().await {
        let Ok(payload) = serde_json::to_string(&serde_json::json!({ "text": chunk })) else {
            continue;
        };
        if tx
            .send(Ok(Event::default().event("delta").data(payload)))
            .await
            .is_err()
        {
            break;
        }
    }
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
    });
    save_conversation(&conv_path, &conv).map_err(|e| e.to_string())?;

    let summaries = list_requirement_summaries(&workspace).map_err(|e| e.to_string())?;
    let ids: Vec<String> = summaries.iter().map(|s| s.id.clone()).collect();
    let review_intent = detect_review_start_intent(&trimmed, &ids);

    let (instance, tool_result, task_id, output_preview) = if review_intent.is_some() {
        let result = tokio::task::spawn_blocking({
            let tools = tools.clone();
            let workspace = workspace.clone();
            let trimmed = trimmed.clone();
            move || run_review_turn(&tools, &workspace, &trimmed)
        })
        .await
        .map_err(|_| "chat_blocking_task_failed".to_string())?
        .map_err(|e| {
            tracing::warn!(error = %e, "requirement review failed (stream)");
            e
        })?;
        let payload = serde_json::to_string(&serde_json::json!({ "text": result.1.output }))
            .map_err(|e| e.to_string())?;
        let _ = sse_tx
            .send(Ok(Event::default().event("delta").data(payload)))
            .await;
        (result.0, result.1, None, None)
    } else {
        let workdir = requirement_agent_workdir(&workspace).map_err(|e| e.to_string())?;
        let prior_rows: Vec<(String, String, u64)> = conv
            .messages
            .iter()
            .map(|m| (m.role.clone(), m.content.clone(), m.created_at_ms))
            .collect();
        let prior = prior_conversation_context(&prior_rows);
        let tool_messages =
            build_requirement_agent_messages(&workspace, &trimmed, &prior).map_err(|e| e.to_string())?;

        let (delta_tx, delta_rx) = tokio::sync::mpsc::channel::<String>(64);
        let forward = tokio::spawn(forward_sse_deltas(delta_rx, sse_tx.clone()));

        let runner = TaskRunner::new(&workspace);
        let result = runner
            .run_code_chat_streaming(
                &tools,
                CodeAgentTaskParams {
                    kind: TaskKind::RequirementAgent,
                    content: task_content_from_user_input(&trimmed),
                    workdir: workdir.clone(),
                    messages: tool_messages,
                    executor_id: Some(employee_id.clone()),
                    parent_task_id: None,
                    context: serde_json::json!({ "employee_id": employee_id }),
                },
                delta_tx,
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
            Ok((task, instance, tool_result)) => {
                let _ = forward.await;
                (instance, tool_result, Some(task.id), task.output_preview.clone())
            }
            Err(raw) => {
                forward.abort();
                return Err(raw);
            }
        }
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
