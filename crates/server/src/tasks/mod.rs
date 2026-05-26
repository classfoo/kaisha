mod detail;
mod model;
mod runner;
mod runtime;
mod store;

pub use detail::{build_task_detail, AgentTaskDetail};

pub use model::{
    AgentTaskRecord, CodeAgentTaskParams, TaskKind, TaskStatus,
};
pub use runner::{
    autonomy_execute_content, autonomy_explore_content, build_rerun_params,     can_rerun_task, hire_task_content, review_context, review_opinion_content, review_pipeline_content,
    review_revision_content, review_summary_content, should_queue_rerun_instead,
    task_content_from_user_input, TaskRunner,
};
pub use runtime::TaskRuntimeRegistry;
pub use store::{filter_tasks, TaskListFilter, TaskStore};

use crate::{i18n, AppState};
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    pub executor_id: Option<String>,
    pub status: Option<TaskStatus>,
    pub kind: Option<TaskKind>,
    pub parent_task_id: Option<String>,
    pub limit: Option<usize>,
}

fn workspace_root(state: &AppState) -> Option<PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

pub async fn list_tasks(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<ListTasksQuery>,
) -> Result<Json<Vec<AgentTaskRecord>>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let store = TaskStore::new(&workspace);
    let tasks = store.list().map_err(|err| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
    })?;
    let filter = TaskListFilter {
        executor_id: query.executor_id,
        status: query.status,
        kind: query.kind,
        parent_task_id: query.parent_task_id,
        limit: query.limit,
    };
    Ok(Json(filter_tasks(tasks, &filter)))
}

pub async fn get_task(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<AgentTaskRecord>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let store = TaskStore::new(&workspace);
    store.load(&task_id).map(Json).map_err(|err| {
        let key = err.to_string();
        if key == "task_not_found" {
            (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(&headers, "task_not_found"),
            )
        } else {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                key,
            )
        }
    })
}

pub async fn get_task_detail(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<AgentTaskDetail>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let store = TaskStore::new(&workspace);
    let task = store.load(&task_id).map_err(|err| {
        let key = err.to_string();
        if key == "task_not_found" {
            (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(&headers, "task_not_found"),
            )
        } else {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                key,
            )
        }
    })?;
    Ok(Json(build_task_detail(&store, task)))
}

pub async fn rerun_task(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<AgentTaskRecord>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let store = TaskStore::new(&workspace);
    let source = store.load(&task_id).map_err(|err| {
        let key = err.to_string();
        if key == "task_not_found" {
            (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(&headers, "task_not_found"),
            )
        } else {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, key)
        }
    })?;

    if !can_rerun_task(&source) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            i18n::msg(&headers, "task_cannot_rerun"),
        ));
    }

    let tasks = store.list().map_err(|err| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
    })?;
    if should_queue_rerun_instead(&source, &tasks) {
        let runner = TaskRunner::new(&workspace);
        let task = runner.queue_rerun(&task_id).map_err(|err| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            )
        })?;
        return Ok(Json(task));
    }

    let params = build_rerun_params(&source);
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workspace_bg = workspace.clone();
    let task_id_bg = task_id.clone();

    let (task, _, _) = tokio::task::spawn_blocking(move || {
        let runner = TaskRunner::new(&workspace_bg);
        runner.rerun_code_chat(&tools, &task_id_bg, params)
    })
    .await
    .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(Json(task))
}

pub async fn stop_task(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<AgentTaskRecord>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let runner = TaskRunner::new(&workspace);
    let task = runner.stop_task(&task_id).map_err(|err| {
        let key = err.to_string();
        if key == "task_not_found" {
            (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(&headers, "task_not_found"),
            )
        } else if key == "task_cannot_stop" {
            (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "task_cannot_stop"),
            )
        } else {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, key)
        }
    })?;
    if let Some(executor_id) = task.executor_id.as_deref() {
        let tools = state.tools.read().expect("tools lock poisoned").clone();
        if let Err(err) = runner.try_drain_queued_reruns(&tools, executor_id) {
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
        }
    }
    Ok(Json(task))
}

#[cfg(test)]
mod tests {
    use super::{
        filter_tasks, AgentTaskRecord, CodeAgentTaskParams, TaskListFilter, TaskStatus, TaskStore,
    };
    use super::model::{truncate_preview, TaskKind};
    use super::store::now_ms;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_workspace() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-task-test-{unique}"))
    }

    #[test]
    fn task_store_roundtrip_and_list() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).expect("create workspace");
        let store = TaskStore::new(&workspace);
        let created = now_ms();
        let task = AgentTaskRecord::new(
            &CodeAgentTaskParams {
                kind: TaskKind::RequirementAgent,
                content: "hello task".into(),
                workdir: workspace.join("requirements"),
                messages: vec![],
                executor_id: Some("alice".into()),
                parent_task_id: None,
                context: serde_json::json!({}),
            },
            "task_test_1".into(),
            created,
        );
        store.save(&task).expect("save task");
        let loaded = store.load("task_test_1").expect("load task");
        assert_eq!(loaded.content, "hello task");
        assert_eq!(loaded.executor_id.as_deref(), Some("alice"));

        let all = store.list().expect("list tasks");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "task_test_1");

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn filter_tasks_by_executor_and_status() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).expect("create workspace");
        let store = TaskStore::new(&workspace);
        for (id, executor, status) in [
            ("t1", "alice", TaskStatus::Completed),
            ("t2", "bob", TaskStatus::Running),
            ("t3", "alice", TaskStatus::Failed),
        ] {
            let mut task = AgentTaskRecord::new(
                &CodeAgentTaskParams {
                    kind: TaskKind::ReviewOpinion,
                    content: id.into(),
                    workdir: workspace.clone(),
                    messages: vec![],
                    executor_id: Some(executor.into()),
                    parent_task_id: None,
                    context: serde_json::json!({}),
                },
                id.into(),
                now_ms(),
            );
            task.status = status;
            store.save(&task).expect("save");
        }
        let all = store.list().expect("list");
        let alice = filter_tasks(
            all.clone(),
            &TaskListFilter {
                executor_id: Some("alice".into()),
                ..Default::default()
            },
        );
        assert_eq!(alice.len(), 2);
        let failed = filter_tasks(
            all,
            &TaskListFilter {
                status: Some(TaskStatus::Failed),
                ..Default::default()
            },
        );
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].id, "t3");

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn truncate_preview_adds_ellipsis_when_needed() {
        let long = "a".repeat(3000);
        let preview = truncate_preview(&long, 100);
        assert!(preview.ends_with('…'));
        assert!(preview.chars().count() <= 101);
    }
}
