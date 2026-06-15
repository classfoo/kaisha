//! Requirement release phase: package the application, start it, and inspect the
//! produced artifacts / run logs. Packaging and starting are delegated to a code
//! agent driven by a suitable operations employee.
//!
//! Release artifacts and reports are kept inside the requirement package:
//! - `requirements/<id>/release/artifacts/` — build outputs
//! - `requirements/<id>/release/output.md`  — packaging summary
//! - `requirements/<id>/release/run.md`      — start/run log

use crate::{
    dev_task_executor::dev_task_workdir,
    i18n,
    requirement::{normalize_requirement_id, requirement_dir, requirement_file_path},
    requirement_agents::{
        pick_employee_for_role, spawn_requirement_agent_task, AgentDispatchWire, AgentTaskSpec,
    },
    tasks::TaskKind,
    tools::driver::ToolChatMessage,
    AppState,
};
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    Json,
};
use serde::Serialize;
use std::{fs, path::Path};

const RELEASE_DIR: &str = "release";
const ARTIFACTS_DIR: &str = "artifacts";
const OUTPUT_FILE: &str = "output.md";
const RUN_FILE: &str = "run.md";

#[derive(Debug, Clone, Serialize)]
pub struct RequirementReleaseWire {
    pub requirement_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_log: Option<String>,
    pub artifacts: Vec<String>,
}

fn release_dir(workspace: &Path, id: &str) -> std::path::PathBuf {
    requirement_dir(workspace, id).join(RELEASE_DIR)
}

fn artifacts_dir(workspace: &Path, id: &str) -> std::path::PathBuf {
    release_dir(workspace, id).join(ARTIFACTS_DIR)
}

fn workspace_root(state: &AppState) -> Option<std::path::PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

fn read_optional(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(path).unwrap_or_default();
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

fn list_artifacts(workspace: &Path, id: &str) -> Vec<String> {
    let dir = artifacts_dir(workspace, id);
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    names
}

fn build_release_wire(workspace: &Path, id: &str) -> RequirementReleaseWire {
    RequirementReleaseWire {
        requirement_id: id.to_string(),
        output: read_optional(&release_dir(workspace, id).join(OUTPUT_FILE)),
        run_log: read_optional(&release_dir(workspace, id).join(RUN_FILE)),
        artifacts: list_artifacts(workspace, id),
    }
}

pub async fn get_release(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RequirementReleaseWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id)
        .map_err(|_| (axum::http::StatusCode::BAD_REQUEST, i18n::msg(&headers, "requirement_id_invalid")))?;
    if !requirement_file_path(&workspace, &id).exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "requirement_not_found"),
        ));
    }
    Ok(Json(build_release_wire(&workspace, &id)))
}

fn build_package_messages(workspace: &Path, requirement_id: &str) -> Vec<ToolChatMessage> {
    let rel = release_dir(workspace, requirement_id);
    let artifacts = artifacts_dir(workspace, requirement_id);
    let output = rel.join(OUTPUT_FILE);
    let prompt = format!(
        r#"You are an operations engineer packaging the application for release.

## Working directory
Your working directory is the product git repository (main repo).

## Task
1. Detect the project's build/packaging tooling (for example npm scripts, cargo, make).
2. Build/package a release artifact for the application. Prefer the project's documented release/build command.
3. Copy the resulting artifact(s) into `{artifacts_dir}` (create the directory if needed).
4. Write a packaging summary to `{output_file}` describing: the commands you ran, where artifacts were placed, the build result (success/failure), and how to install/run the package.
5. Reply briefly with the outcome.

Use absolute paths exactly as given above when writing outside the repository. Do not only describe intent — perform the build and write the files."#,
        artifacts_dir = artifacts.to_string_lossy(),
        output_file = output.to_string_lossy(),
    );
    vec![ToolChatMessage {
        role: "user".to_string(),
        content: prompt,
    }]
}

fn build_start_messages(workspace: &Path, requirement_id: &str) -> Vec<ToolChatMessage> {
    let run = release_dir(workspace, requirement_id).join(RUN_FILE);
    let prompt = format!(
        r#"You are an operations engineer starting the application to verify it runs.

## Working directory
Your working directory is the product git repository (main repo).

## Task
1. Detect how the application is started (for example a dev/start script or a built binary).
2. Start the application in a way that lets you confirm it boots successfully (run a short-lived smoke check; do NOT block forever — stop the process after confirming startup).
3. Write a run log to `{run_file}` describing: the start command, startup output, whether it started successfully, and any errors.
4. Reply briefly with whether the application started successfully.

Use the absolute path exactly as given above. Do not only describe intent — actually attempt to start the app and write the log."#,
        run_file = run.to_string_lossy(),
    );
    vec![ToolChatMessage {
        role: "user".to_string(),
        content: prompt,
    }]
}

async fn dispatch_release_agent(
    headers: &HeaderMap,
    state: &AppState,
    id: String,
    content: String,
    messages_builder: impl FnOnce(&Path, &str) -> Vec<ToolChatMessage>,
) -> Result<Json<AgentDispatchWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(headers, "workspace_not_configured"),
        ));
    };
    let id = normalize_requirement_id(&id)
        .map_err(|_| (axum::http::StatusCode::BAD_REQUEST, i18n::msg(headers, "requirement_id_invalid")))?;
    if !requirement_file_path(&workspace, &id).exists() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(headers, "requirement_not_found"),
        ));
    }

    let Some(employee) = pick_employee_for_role(&workspace, "operations") else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(headers, "requirement_no_employees"),
        ));
    };

    // Ensure the release directory exists so the agent can write into it.
    let _ = fs::create_dir_all(artifacts_dir(&workspace, &id));

    let messages = messages_builder(&workspace, &id);
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workdir = dev_task_workdir(&workspace);
    spawn_requirement_agent_task(
        &workspace,
        &tools,
        &employee.id,
        AgentTaskSpec {
            kind: TaskKind::WorkTaskExecute,
            content,
            workdir,
            messages,
            context: serde_json::json!({ "requirement_id": id }),
        },
        |_ws| {},
    );

    Ok(Json(AgentDispatchWire::from_employee(&employee)))
}

pub async fn package_release(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<AgentDispatchWire>, (axum::http::StatusCode, String)> {
    let content = format!("Package application for release `{id}`");
    dispatch_release_agent(&headers, &state, id, content, build_package_messages).await
}

pub async fn start_release(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<AgentDispatchWire>, (axum::http::StatusCode, String)> {
    let content = format!("Start application for release `{id}`");
    dispatch_release_agent(&headers, &state, id, content, build_start_messages).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-release-{unique}"))
    }

    #[test]
    fn release_wire_reads_output_and_artifacts() {
        let workspace = temp_workspace();
        let rel = release_dir(&workspace, "auth");
        fs::create_dir_all(rel.join(ARTIFACTS_DIR)).unwrap();
        fs::write(rel.join(OUTPUT_FILE), "# Built\n\nok").unwrap();
        fs::write(rel.join(ARTIFACTS_DIR).join("app.tar.gz"), "x").unwrap();

        let wire = build_release_wire(&workspace, "auth");
        assert!(wire.output.unwrap().contains("Built"));
        assert_eq!(wire.artifacts, vec!["app.tar.gz".to_string()]);
        assert!(wire.run_log.is_none());

        let _ = fs::remove_dir_all(&workspace);
    }
}
