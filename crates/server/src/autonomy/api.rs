use crate::employee_chat::{conversation_path, load_conversation, save_conversation, new_message_id, ConversationFile, StoredMessage};
use crate::tools::manager::ToolManager;
use crate::AppState;
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    response::sse::{Event, Sse},
    Json,
};
use serde::Serialize;
use std::{
    convert::Infallible,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug, Clone, Serialize)]
pub struct AutonomyStatusWire {
    pub enabled: bool,
    pub worker_count: usize,
    pub task_counts: std::collections::HashMap<String, usize>,
    pub plan_count: usize,
}

pub async fn get_autonomy_status(
    State(state): State<AppState>,
) -> Json<AutonomyStatusWire> {
    if let Some(runtime) = &state.autonomy {
        let workers = runtime.list_workers().await.unwrap_or_default();
        let tasks = runtime.list_tasks().await.unwrap_or_default();
        let plans = runtime.list_plans().await.unwrap_or_default();

        let mut task_counts = std::collections::HashMap::new();
        for task in &tasks {
            *task_counts.entry(task.status.as_str().to_string()).or_insert(0) += 1;
        }

        Json(AutonomyStatusWire {
            enabled: true,
            worker_count: workers.len(),
            task_counts,
            plan_count: plans.len(),
        })
    } else {
        Json(AutonomyStatusWire {
            enabled: false,
            worker_count: 0,
            task_counts: Default::default(),
            plan_count: 0,
        })
    }
}

pub async fn run_autonomy_tick_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<AutonomyStatusWire>, (axum::http::StatusCode, String)> {
    let Some(runtime) = state.autonomy.clone() else {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            crate::i18n::msg(&headers, "autonomy_not_enabled"),
        ));
    };
    runtime.notify();
    Ok(Json(AutonomyStatusWire {
        enabled: true,
        worker_count: 0,
        task_counts: Default::default(),
        plan_count: 0,
    }))
}

pub async fn list_employee_todos_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(employee_id): AxumPath<String>,
) -> Result<Json<crate::employee_todo::EmployeeTodoFile>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            crate::i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    crate::employee_todo::load_todos(&workspace, &employee_id)
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn run_employee_autonomy_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(employee_id): AxumPath<String>,
) -> Result<Json<crate::employee_todo::EmployeeTodoFile>, (axum::http::StatusCode, String)> {
    let Some(runtime) = &state.autonomy else {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            crate::i18n::msg(&headers, "autonomy_not_enabled"),
        ));
    };
    runtime.notify();
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            crate::i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    crate::employee_todo::load_todos(&workspace, &employee_id)
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn run_employee_autonomy_explore_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(employee_id): AxumPath<String>,
) -> Result<Json<crate::employee_todo::EmployeeTodoFile>, (axum::http::StatusCode, String)> {
    let Some(runtime) = &state.autonomy else {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            crate::i18n::msg(&headers, "autonomy_not_enabled"),
        ));
    };
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            crate::i18n::msg(&headers, "workspace_not_configured"),
        ));
    };

    let tools_arc = state.tools.clone();
    let workspace_clone = workspace.clone();
    let employee_id_clone = employee_id.clone();
    let headers_clone = headers.clone();

    // Create a task_process message and run explore with streaming events,
    // then persist events to conversation.json
    let result = run_autonomy_task_with_conversation(
        &workspace_clone,
        &employee_id_clone,
        &headers_clone,
        tools_arc,
        AutonomyTaskKind::Explore,
    )
    .await?;

    runtime.notify();

    Ok(Json(result))
}

pub async fn list_tasks_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::autonomy::task::Task>>, (axum::http::StatusCode, String)> {
    let Some(runtime) = &state.autonomy else {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "autonomy_not_enabled".to_string(),
        ));
    };
    let tasks = runtime.list_tasks().await.map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(tasks))
}

pub async fn list_plans_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::autonomy::plan::Plan>>, (axum::http::StatusCode, String)> {
    let Some(runtime) = &state.autonomy else {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "autonomy_not_enabled".to_string(),
        ));
    };
    let plans = runtime.list_plans().await.map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(plans))
}

pub async fn list_workers_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::autonomy::worker::Worker>>, (axum::http::StatusCode, String)> {
    let Some(runtime) = &state.autonomy else {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "autonomy_not_enabled".to_string(),
        ));
    };
    let workers = runtime.list_workers().await.map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(workers))
}

fn workspace_root(state: &AppState) -> Option<std::path::PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

/// Which autonomy task to run (explore or execute).
enum AutonomyTaskKind {
    Explore,
}

/// Runs an autonomy task (explore/execute) with streaming events,
/// creates a task_process message, persists events to conversation.json,
/// and returns the resulting EmployeeTodoFile.
async fn run_autonomy_task_with_conversation(
    workspace: &std::path::Path,
    employee_id: &str,
    headers: &HeaderMap,
    tools_arc: Arc<std::sync::RwLock<ToolManager>>,
    kind: AutonomyTaskKind,
) -> Result<crate::employee_todo::EmployeeTodoFile, (axum::http::StatusCode, String)> {
    let tools: ToolManager = {
        let guard = tools_arc.read();
        guard.map(|g| g.clone()).map_err(|_| "tools lock poisoned".to_string())
    }.map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Load conversation and create task_process message
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
        tracing::warn!(error = %e, "failed to save conversation before autonomy task");
    }

    // Create a channel for streaming events
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<crate::tools::driver::ChatStreamEvent>(32);

    // Collect events and persist to conversation
    let conv_path_clone = conv_path.clone();
    let collect_handle = tokio::spawn(async move {
        let mut collected_events: Vec<serde_json::Value> = Vec::new();
        let mut events_since_save = 0;
        let save_interval = crate::employee_chat::STREAM_PERSIST_INTERVAL;

        // Reload conversation from disk (the task_process message was already saved)
        let mut conv = load_conversation(&conv_path_clone).unwrap_or_else(|_| ConversationFile {
            version: 1,
            messages: vec![],
        });

        while let Some(event) = event_rx.recv().await {
            let event_value = serde_json::to_value(&event).ok();

            // Store event for persistence
            if let Some(val) = event_value {
                collected_events.push(val);
                events_since_save += 1;

                // Update the task_process message with collected events
                if task_process_idx < conv.messages.len() {
                    conv.messages[task_process_idx].stream_events = Some(collected_events.clone());
                }

                // Periodic save
                if events_since_save >= save_interval {
                    if let Err(e) = save_conversation(&conv_path_clone, &conv) {
                        tracing::warn!(error = %e, "failed to save conversation during autonomy task");
                    }
                    events_since_save = 0;
                }
            }
        }

        // Final save
        if let Err(e) = save_conversation(&conv_path_clone, &conv) {
            tracing::warn!(error = %e, "failed to save conversation at end of autonomy task");
        }
    });

    // Run the appropriate autonomy task
    let headers_ref = headers.clone();
    let workspace_ref = workspace.to_path_buf();
    let employee_id_ref = employee_id.to_string();
    let task_result = match kind {
        AutonomyTaskKind::Explore => {
            crate::autonomy::explore::run_explore_streaming(
                &workspace_ref,
                &tools,
                &employee_id_ref,
                &headers_ref,
                event_tx,
            ).await
        }
    };

    // Wait for the collector to finish
    let _ = collect_handle.await;

    // Reload conversation and update task_process message
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

    // Load and return todos
    crate::employee_todo::load_todos(workspace, employee_id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn run_employee_autonomy_explore_stream_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(employee_id): AxumPath<String>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let Some(workspace) = workspace_root(&state) else {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(1);
        let err_msg = crate::i18n::msg(&headers, "workspace_not_configured");
        let _ = tx.send(Ok(Event::default().event("error").data(serde_json::json!({ "message": err_msg }).to_string()))).await;
        return Sse::new(ReceiverStream::new(rx)).keep_alive(
            axum::response::sse::KeepAlive::new().interval(Duration::from_secs(20)),
        );
    };

    let (sse_tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);

    let tools_arc = state.tools.clone();
    let headers_clone = headers.clone();
    let workspace_clone = workspace.clone();
    let employee_id_clone = employee_id.clone();

    tokio::spawn(async move {
        let tools: Result<ToolManager, String> = {
            let guard = tools_arc.read();
            guard.map(|g| g.clone()).map_err(|_| "tools lock poisoned".to_string())
        };
        let tools = match tools {
            Ok(t) => t,
            Err(err_msg) => {
                let _ = sse_tx.send(Ok(Event::default().event("error").data(serde_json::json!({ "message": err_msg }).to_string()))).await;
                return;
            }
        };

        // Load conversation and create task_process message
        let conv_path = conversation_path(&workspace_clone, &employee_id_clone);
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
            tracing::warn!(error = %e, "failed to save conversation before streaming");
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel::<crate::tools::driver::ChatStreamEvent>(32);

        // Forward ChatStreamEvent to SSE events and persist to conversation
        let sse_tx_clone = sse_tx.clone();
        let conv_path_for_spawn = conv_path.clone();
        tokio::spawn(async move {
            forward_sse_events_with_persistence(
                event_rx,
                sse_tx_clone,
                conv_path_for_spawn,
                conv,
                task_process_idx,
                crate::employee_chat::STREAM_PERSIST_INTERVAL,
            ).await;
        });

        // Run the streaming explore
        let result = crate::autonomy::explore::run_explore_streaming(
            &workspace_clone,
            &tools,
            &employee_id_clone,
            &headers_clone,
            event_tx,
        ).await;

        // Update task_process with final status and reload conv
        let mut conv = load_conversation(&conv_path).unwrap_or_else(|_| ConversationFile {
            version: 1,
            messages: vec![],
        });
        match &result {
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

        // Send final event
        match result {
            Ok(task) => {
                let done_payload = serde_json::json!({
                    "task_id": task.id,
                    "status": "completed",
                    "employee_id": employee_id_clone,
                }).to_string();
                let _ = sse_tx.send(Ok(Event::default().event("done").data(done_payload))).await;
            }
            Err(e) => {
                let err_payload = serde_json::json!({
                    "status": "failed",
                    "error": e.to_string(),
                    "employee_id": employee_id_clone,
                }).to_string();
                let _ = sse_tx.send(Ok(Event::default().event("error").data(err_payload))).await;
            }
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(
        axum::response::sse::KeepAlive::new().interval(Duration::from_secs(20)),
    )
}

pub async fn run_employee_autonomy_run_stream_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(employee_id): AxumPath<String>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let Some(workspace) = workspace_root(&state) else {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(1);
        let err_msg = crate::i18n::msg(&headers, "workspace_not_configured");
        let _ = tx.send(Ok(Event::default().event("error").data(serde_json::json!({ "message": err_msg }).to_string()))).await;
        return Sse::new(ReceiverStream::new(rx)).keep_alive(
            axum::response::sse::KeepAlive::new().interval(Duration::from_secs(20)),
        );
    };

    let (sse_tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);

    let tools_arc = state.tools.clone();
    let headers_clone = headers.clone();
    let workspace_clone = workspace.clone();
    let employee_id_clone = employee_id.clone();

    tokio::spawn(async move {
        let tools: Result<ToolManager, String> = {
            let guard = tools_arc.read();
            guard.map(|g| g.clone()).map_err(|_| "tools lock poisoned".to_string())
        };
        let tools = match tools {
            Ok(t) => t,
            Err(err_msg) => {
                let _ = sse_tx.send(Ok(Event::default().event("error").data(serde_json::json!({ "message": err_msg }).to_string()))).await;
                return;
            }
        };

        // Load conversation and create task_process message
        let conv_path = conversation_path(&workspace_clone, &employee_id_clone);
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
            tracing::warn!(error = %e, "failed to save conversation before streaming");
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel::<crate::tools::driver::ChatStreamEvent>(32);

        // Forward ChatStreamEvent to SSE events and persist to conversation
        let sse_tx_clone = sse_tx.clone();
        let conv_path_for_spawn = conv_path.clone();
        tokio::spawn(async move {
            forward_sse_events_with_persistence(
                event_rx,
                sse_tx_clone,
                conv_path_for_spawn,
                conv,
                task_process_idx,
                crate::employee_chat::STREAM_PERSIST_INTERVAL,
            ).await;
        });

        // Run the streaming execute
        let result = crate::autonomy::execute::run_execute_streaming(
            &workspace_clone,
            &tools,
            &employee_id_clone,
            &headers_clone,
            event_tx,
        ).await;

        // Update task_process with final status and reload conv
        let mut conv = load_conversation(&conv_path).unwrap_or_else(|_| ConversationFile {
            version: 1,
            messages: vec![],
        });
        match &result {
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

        // Send final event
        match result {
            Ok(task) => {
                let done_payload = serde_json::json!({
                    "task_id": task.id,
                    "status": "completed",
                    "employee_id": employee_id_clone,
                }).to_string();
                let _ = sse_tx.send(Ok(Event::default().event("done").data(done_payload))).await;
            }
            Err(e) => {
                let err_payload = serde_json::json!({
                    "status": "failed",
                    "error": e.to_string(),
                    "employee_id": employee_id_clone,
                }).to_string();
                let _ = sse_tx.send(Ok(Event::default().event("error").data(err_payload))).await;
            }
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(
        axum::response::sse::KeepAlive::new().interval(Duration::from_secs(20)),
    )
}

async fn forward_sse_events_with_persistence(
    mut event_rx: tokio::sync::mpsc::Receiver<crate::tools::driver::ChatStreamEvent>,
    tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
    conv_path: PathBuf,
    mut conv: ConversationFile,
    task_process_msg_idx: usize,
    save_interval: usize,
) {
    use crate::tools::driver::ChatStreamEvent;
    let mut collected_events: Vec<serde_json::Value> = Vec::new();
    let mut events_since_save = 0;

    while let Some(event) = event_rx.recv().await {
        // Serialize event as JSON value for storage
        let event_value = serde_json::to_value(&event).ok();

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
                if let Err(e) = save_conversation(&conv_path, &conv) {
                    tracing::warn!(error = %e, "failed to save conversation during streaming");
                }
                events_since_save = 0;
            }
        }
    }

    // Final save
    if let Err(e) = save_conversation(&conv_path, &conv) {
        tracing::warn!(error = %e, "failed to save conversation at end of streaming");
    }
}
