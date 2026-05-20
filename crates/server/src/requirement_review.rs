use crate::{
    employee::{append_employee_memory, list_employee_records, EmployeeRecord},
    i18n,
    requirement::{
        load_requirement_detail, normalize_requirement_id, requirement_dir, RequirementPhase,
        REQUIREMENT_FILE,
    },
    tools::{driver::ToolChatMessage, manager::ToolManager},
    work_rules::{duty_for_phase, load_work_rules, resolve_role_key, WorkRulesFile},
    AppState,
};
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

const REVIEW_DIR: &str = "review";
const OPINIONS_DIR: &str = "opinions";
const STATE_FILE: &str = "state.json";
const SUMMARY_FILE: &str = "summary.md";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewConclusion {
    Adopt,
    Supplement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewStateFile {
    pub requirement_id: String,
    pub status: ReviewStatus,
    pub started_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<ReviewConclusion>,
    pub participants: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewOpinionWire {
    pub employee_id: String,
    pub employee_name: String,
    pub role: String,
    pub role_key: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementReviewWire {
    pub requirement_id: String,
    pub status: ReviewStatus,
    pub started_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    pub conclusion: Option<ReviewConclusion>,
    pub participants: Vec<String>,
    pub opinions: Vec<ReviewOpinionWire>,
    pub summary: Option<String>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn review_root(workspace: &Path, requirement_id: &str) -> PathBuf {
    requirement_dir(workspace, requirement_id).join(REVIEW_DIR)
}

fn opinions_root(workspace: &Path, requirement_id: &str) -> PathBuf {
    review_root(workspace, requirement_id).join(OPINIONS_DIR)
}

fn state_path(workspace: &Path, requirement_id: &str) -> PathBuf {
    review_root(workspace, requirement_id).join(STATE_FILE)
}

fn summary_path(workspace: &Path, requirement_id: &str) -> PathBuf {
    review_root(workspace, requirement_id).join(SUMMARY_FILE)
}

pub fn requirement_workdir(workspace: &Path, requirement_id: &str) -> PathBuf {
    requirement_dir(workspace, requirement_id)
}

pub fn detect_review_start_intent(input: &str, known_ids: &[String]) -> Option<String> {
    let lower = input.to_lowercase();
    let triggers = [
        "进入需求评审",
        "开始需求评审",
        "启动需求评审",
        "需求评审",
        "start requirement review",
        "enter requirement review",
        "run requirement review",
    ];
    if !triggers.iter().any(|t| lower.contains(t)) {
        return None;
    }
    for id in known_ids {
        if input.contains(id.as_str()) {
            return Some(id.clone());
        }
    }
    known_ids.first().cloned()
}

fn set_requirement_phase(workspace: &Path, requirement_id: &str, phase: RequirementPhase) -> anyhow::Result<()> {
    let _detail = load_requirement_detail(workspace, requirement_id)?;
    let file_path = requirement_dir(workspace, requirement_id).join(REQUIREMENT_FILE);
    let raw = fs::read_to_string(&file_path)?;
    let (mut meta, content) = crate::requirement::parse_requirement_md(&raw)?;
    meta.phase = phase;
    meta.updated_at_ms = now_ms();
    fs::write(
        &file_path,
        crate::requirement::format_requirement_md(&meta, &content),
    )?;
    Ok(())
}

pub fn start_review(workspace: &Path, requirement_id: &str) -> anyhow::Result<ReviewStateFile> {
    let id = normalize_requirement_id(requirement_id)?;
    let _ = load_requirement_detail(workspace, &id)?;
    let employees = list_employee_records(workspace)?;
    if employees.is_empty() {
        anyhow::bail!("review_no_employees");
    }
    fs::create_dir_all(opinions_root(workspace, &id))?;
    let state = ReviewStateFile {
        requirement_id: id.clone(),
        status: ReviewStatus::InProgress,
        started_at_ms: now_ms(),
        completed_at_ms: None,
        conclusion: None,
        participants: employees.iter().map(|e| e.id.clone()).collect(),
    };
    fs::write(state_path(workspace, &id), serde_json::to_string_pretty(&state)?)?;
    set_requirement_phase(workspace, &id, RequirementPhase::Review)?;
    Ok(state)
}

fn load_state(workspace: &Path, requirement_id: &str) -> anyhow::Result<ReviewStateFile> {
    let path = state_path(workspace, requirement_id);
    if !path.exists() {
        anyhow::bail!("review_not_started");
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn save_state(workspace: &Path, state: &ReviewStateFile) -> anyhow::Result<()> {
    fs::write(
        state_path(workspace, &state.requirement_id),
        serde_json::to_string_pretty(state)?,
    )?;
    Ok(())
}

fn load_opinions(workspace: &Path, requirement_id: &str, employees: &[EmployeeRecord]) -> Vec<ReviewOpinionWire> {
    let dir = opinions_root(workspace, requirement_id);
    let mut out = Vec::new();
    for emp in employees {
        let path = dir.join(format!("{}.md", emp.id));
        let content = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };
        if content.trim().is_empty() {
            continue;
        }
        let rules = load_work_rules(workspace).ok();
        let role_key = rules
            .as_ref()
            .and_then(|r| resolve_role_key(r, &emp.role));
        out.push(ReviewOpinionWire {
            employee_id: emp.id.clone(),
            employee_name: emp.name.clone(),
            role: emp.role.clone(),
            role_key,
            content,
        });
    }
    out
}

pub fn load_review_wire(workspace: &Path, requirement_id: &str) -> anyhow::Result<RequirementReviewWire> {
    let id = normalize_requirement_id(requirement_id)?;
    let state = load_state(workspace, &id)?;
    let employees = list_employee_records(workspace)?;
    let opinions = load_opinions(workspace, &id, &employees);
    let summary = summary_path(workspace, &id)
        .exists()
        .then(|| fs::read_to_string(summary_path(workspace, &id)).ok())
        .flatten();
    Ok(RequirementReviewWire {
        requirement_id: id,
        status: state.status,
        started_at_ms: state.started_at_ms,
        completed_at_ms: state.completed_at_ms,
        conclusion: state.conclusion,
        participants: state.participants,
        opinions,
        summary,
    })
}

fn build_reviewer_messages(
    rules: &WorkRulesFile,
    employee: &EmployeeRecord,
    role_key: Option<&str>,
    requirement_id: &str,
    requirement_title: &str,
    requirement_content: &str,
) -> Vec<ToolChatMessage> {
    let role_label = role_key
        .and_then(|k| rules.roles.get(k))
        .map(|r| r.display_name.as_str())
        .unwrap_or(employee.role.as_str());
    let duty = role_key
        .map(|k| duty_for_phase(rules, k, "review"))
        .unwrap_or_else(|| "Review the requirement from your professional perspective.".to_string());
    let system = format!(
        r#"You are participating in a formal requirement review as **{role_label}** ({employee_name}).

## Working directory
This directory is the requirement `{requirement_id}` package. The requirement body is in `{REQUIREMENT_FILE}`.

## Your duty in the review phase
{duty}

## Task
1. Read `{REQUIREMENT_FILE}` and any related files.
2. Write your review opinion to `review/opinions/{employee_id}.md` (create directories if needed).
3. Use Markdown with sections: Summary, Findings, Risks, Recommendation (approve / needs change).
4. Reply briefly confirming the file you wrote.

Be concrete and actionable. Do not only describe what you would do — write the opinion file."#,
        role_label = role_label,
        employee_name = employee.name,
        requirement_id = requirement_id,
        REQUIREMENT_FILE = REQUIREMENT_FILE,
        duty = duty,
        employee_id = employee.id,
    );
    vec![
        ToolChatMessage {
            role: "system".to_string(),
            content: system,
        },
        ToolChatMessage {
            role: "user".to_string(),
            content: format!(
                "Review requirement **{requirement_title}** (`{requirement_id}`) now.\n\n---\n\n{requirement_content}"
            ),
        },
    ]
}

fn build_summary_messages(
    requirement_id: &str,
    requirement_title: &str,
    opinions: &[ReviewOpinionWire],
) -> Vec<ToolChatMessage> {
    let mut catalog = String::new();
    for op in opinions {
        catalog.push_str(&format!(
            "### {} ({}) — {}\n{}\n\n",
            op.employee_name, op.role, op.employee_id, op.content
        ));
    }
    if catalog.is_empty() {
        catalog.push_str("(no opinions recorded)\n");
    }
    let system = format!(
        r#"You are the review facilitator for requirement `{requirement_id}`.

## Working directory
Requirement package directory. Opinions are under `review/opinions/`.

## Task
1. Read all reviewer opinions below.
2. Write `review/summary.md` containing:
   - Executive summary
   - Consolidated findings
   - **Conclusion** — exactly one of:
     - `adopt` — requirement is accepted to proceed (move toward confirm phase)
     - `supplement` — requirement needs more information or changes (send back to collection)
3. End the summary file with a line: `CONCLUSION: adopt` or `CONCLUSION: supplement`
4. Reply with the same conclusion line in your message."#,
        requirement_id = requirement_id,
    );
    vec![
        ToolChatMessage {
            role: "system".to_string(),
            content: system,
        },
        ToolChatMessage {
            role: "user".to_string(),
            content: format!(
                "Synthesize review for **{requirement_title}** (`{requirement_id}`).\n\n## Opinions\n\n{catalog}"
            ),
        },
    ]
}

fn parse_conclusion(text: &str) -> Option<ReviewConclusion> {
    let lower = text.to_lowercase();
    if lower.contains("conclusion: supplement") || lower.contains("conclusion:supplement") {
        return Some(ReviewConclusion::Supplement);
    }
    if lower.contains("conclusion: adopt") || lower.contains("conclusion:adopt") {
        return Some(ReviewConclusion::Adopt);
    }
    if lower.contains("supplement") || lower.contains("补充") {
        return Some(ReviewConclusion::Supplement);
    }
    if lower.contains("adopt") || lower.contains("采纳") {
        return Some(ReviewConclusion::Adopt);
    }
    None
}

fn ensure_opinion_file(
    workspace: &Path,
    requirement_id: &str,
    employee_id: &str,
    agent_output: &str,
) -> anyhow::Result<String> {
    let path = opinions_root(workspace, requirement_id).join(format!("{employee_id}.md"));
    if path.exists() {
        return Ok(fs::read_to_string(&path)?);
    }
    fs::create_dir_all(path.parent().expect("parent"))?;
    let body = if agent_output.trim().is_empty() {
        "_No opinion file produced by agent._".to_string()
    } else {
        agent_output.trim().to_string()
    };
    fs::write(&path, &body)?;
    Ok(body)
}

fn memory_section_review(requirement_id: &str, title: &str, body: &str) -> String {
    format!("Requirement `{requirement_id}` ({title})\n\n{body}")
}

pub fn run_requirement_review(
    workspace: &Path,
    tools: &ToolManager,
    requirement_id: &str,
) -> anyhow::Result<RequirementReviewWire> {
    let id = normalize_requirement_id(requirement_id)?;
    let _ = start_review(workspace, &id);
    let rules = load_work_rules(workspace)?;
    let detail = load_requirement_detail(workspace, &id)?;
    let workdir = requirement_workdir(workspace, &id);
    let employees = list_employee_records(workspace)?;

    for employee in &employees {
        let role_key = resolve_role_key(&rules, &employee.role);
        let messages = build_reviewer_messages(
            &rules,
            employee,
            role_key.as_deref(),
            &id,
            &detail.title,
            &detail.content,
        );
        let (_instance, result) = tools.execute_code_chat(&workdir, &messages)?;
        let opinion = ensure_opinion_file(workspace, &id, &employee.id, &result.output)?;
        let section = format!("Review opinion — {}", now_ms());
        append_employee_memory(
            workspace,
            &employee.id,
            &section,
            &memory_section_review(&id, &detail.title, &opinion),
        )?;
    }

    let opinions = load_opinions(workspace, &id, &employees);
    let summary_messages = build_summary_messages(&id, &detail.title, &opinions);
    let (_summary_inst, summary_result) = tools.execute_code_chat(&workdir, &summary_messages)?;
    let summary_text = if summary_path(workspace, &id).exists() {
        fs::read_to_string(summary_path(workspace, &id))?
    } else {
        let t = summary_result.output.clone();
        fs::write(summary_path(workspace, &id), &t)?;
        t
    };

    let conclusion = parse_conclusion(&summary_text)
        .or_else(|| parse_conclusion(&summary_result.output))
        .unwrap_or(ReviewConclusion::Supplement);

    let next_phase = match conclusion {
        ReviewConclusion::Adopt => RequirementPhase::Confirm,
        ReviewConclusion::Supplement => RequirementPhase::Collection,
    };
    set_requirement_phase(workspace, &id, next_phase)?;

    let mut state = load_state(workspace, &id)?;
    state.status = ReviewStatus::Completed;
    state.completed_at_ms = Some(now_ms());
    state.conclusion = Some(conclusion);
    save_state(workspace, &state)?;

    let memory_body = format!(
        "Review completed.\n\nConclusion: {:?}\n\n{summary_text}",
        state.conclusion
    );
    for employee in &employees {
        append_employee_memory(
            workspace,
            &employee.id,
            &format!("Review summary — {}", now_ms()),
            &memory_section_review(&id, &detail.title, &memory_body),
        )?;
    }

    load_review_wire(workspace, &id)
}

fn workspace_root(state: &AppState) -> Option<PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

pub async fn get_requirement_review(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RequirementReviewWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    load_review_wire(&workspace, &id)
        .map(Json)
        .map_err(map_review_err(&headers))
}

pub async fn start_requirement_review(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RequirementReviewWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    start_review(&workspace, &id).map_err(map_review_err(&headers))?;
    load_review_wire(&workspace, &id)
        .map(Json)
        .map_err(map_review_err(&headers))
}

pub async fn run_review_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RequirementReviewWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workspace_path = workspace;
    let req_id = id;
    tokio::task::spawn_blocking(move || run_requirement_review(&workspace_path, &tools, &req_id))
        .await
        .map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                i18n::msg(&headers, "chat_blocking_task_failed"),
            )
        })?
        .map_err(map_review_err(&headers))
        .map(Json)
}

fn map_review_err(
    headers: &HeaderMap,
) -> impl Fn(anyhow::Error) -> (axum::http::StatusCode, String) + '_ {
    move |err| {
        let key = err.to_string();
        let known = [
            "requirement_not_found",
            "review_not_started",
            "review_no_employees",
            "review_already_completed",
            "no_enabled_coding_tool",
        ];
        if key == "requirement_not_found" || key == "review_not_started" {
            return (
                axum::http::StatusCode::NOT_FOUND,
                i18n::msg(headers, &key),
            );
        }
        if key == "no_enabled_coding_tool" {
            return (
                axum::http::StatusCode::CONFLICT,
                i18n::msg(headers, "chat_tool_missing"),
            );
        }
        if known.contains(&key.as_str()) {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(headers, &key),
            );
        }
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_review_intent_with_id() {
        let ids = vec!["sso-login".to_string()];
        let id = detect_review_start_intent("请对 sso-login 进入需求评审", &ids);
        assert_eq!(id.as_deref(), Some("sso-login"));
    }

    #[test]
    fn parses_conclusion_from_summary() {
        assert_eq!(
            parse_conclusion("done\n\nCONCLUSION: adopt"),
            Some(ReviewConclusion::Adopt)
        );
        assert_eq!(
            parse_conclusion("CONCLUSION: supplement"),
            Some(ReviewConclusion::Supplement)
        );
    }
}
