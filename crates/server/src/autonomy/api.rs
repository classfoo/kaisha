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
    time::Duration,
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
    let todos = tokio::task::spawn_blocking({
        let workspace = workspace.clone();
        let employee_id = employee_id.clone();
        let headers = headers.clone();
        move || {
            let tools = tools_arc
                .read()
                .map_err(|e| anyhow::anyhow!("tools lock poisoned: {}", e))?;
            crate::autonomy::explore::run_explore(&workspace, &tools, &employee_id, &headers)
        }
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    runtime.notify();

    Ok(Json(todos))
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

        let (event_tx, event_rx) = tokio::sync::mpsc::channel::<crate::tools::driver::ChatStreamEvent>(32);

        // Forward ChatStreamEvent to SSE events
        let sse_tx_clone = sse_tx.clone();
        tokio::spawn(async move {
            forward_sse_events(event_rx, sse_tx_clone).await;
        });

        // Run the streaming explore
        let result = crate::autonomy::explore::run_explore_streaming(
            &workspace_clone,
            &tools,
            &employee_id_clone,
            &headers_clone,
            event_tx,
        ).await;

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

        let (event_tx, event_rx) = tokio::sync::mpsc::channel::<crate::tools::driver::ChatStreamEvent>(32);

        // Forward ChatStreamEvent to SSE events
        let sse_tx_clone = sse_tx.clone();
        tokio::spawn(async move {
            forward_sse_events(event_rx, sse_tx_clone).await;
        });

        // Run the streaming execute
        let result = crate::autonomy::execute::run_execute_streaming(
            &workspace_clone,
            &tools,
            &employee_id_clone,
            &headers_clone,
            event_tx,
        ).await;

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

async fn forward_sse_events(
    mut event_rx: tokio::sync::mpsc::Receiver<crate::tools::driver::ChatStreamEvent>,
    tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) {
    use crate::tools::driver::ChatStreamEvent;
    while let Some(event) = event_rx.recv().await {
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
        if tx
            .send(Ok(Event::default().event(event_name).data(data)))
            .await
            .is_err()
        {
            break;
        }
    }
}
