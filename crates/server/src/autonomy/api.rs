use crate::AppState;
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    Json,
};
use serde::Serialize;

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
