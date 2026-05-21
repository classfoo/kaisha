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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpinionItemStatus {
    Pending,
    InProgress,
    Revising,
    Completed,
    Abandoned,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTask {
    Opinion,
    Revise,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_reviewer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_task: Option<ReviewTask>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub abandoned_participants: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewOpinionWire {
    pub employee_id: String,
    pub employee_name: String,
    pub role: String,
    pub role_key: Option<String>,
    pub status: OpinionItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReviewTally {
    pub passed: u32,
    pub failed: u32,
    pub pending: u32,
    pub undecided: u32,
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
    pub passed_count: u32,
    pub failed_count: u32,
    pub pending_count: u32,
    pub undecided_count: u32,
    pub abandoned_count: u32,
    pub overall_passed: bool,
}

const MAX_REVISION_ROUNDS: u32 = 8;

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
        current_reviewer_id: None,
        current_task: None,
        abandoned_participants: Vec::new(),
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

pub fn parse_opinion_passed(content: &str) -> Option<bool> {
    let lower = content.to_lowercase();
    if lower.contains("recommendation: needs change")
        || lower.contains("recommendation:needs change")
        || lower.contains("recommendation: reject")
        || lower.contains("recommendation:reject")
        || lower.contains("needs change")
        || lower.contains("need change")
        || lower.contains("conclusion: supplement")
        || lower.contains("不通过")
        || lower.contains("需修改")
        || lower.contains("需补充")
        || lower.contains("❌")
    {
        return Some(false);
    }
    if lower.contains("recommendation: approve")
        || lower.contains("recommendation:approve")
        || lower.contains("conclusion: approve")
        || lower.contains("✅")
        || (lower.contains("通过") && !lower.contains("不通过"))
        || (lower.contains("采纳") && !lower.contains("不采纳"))
    {
        return Some(true);
    }
    None
}

pub fn review_tally(opinions: &[ReviewOpinionWire]) -> ReviewTally {
    let mut tally = ReviewTally {
        passed: 0,
        failed: 0,
        pending: 0,
        undecided: 0,
    };
    for op in opinions {
        match op.status {
            OpinionItemStatus::Abandoned => {}
            OpinionItemStatus::Pending | OpinionItemStatus::InProgress | OpinionItemStatus::Revising => {
                tally.pending += 1;
            }
            OpinionItemStatus::Completed => match op.passed {
                Some(true) => tally.passed += 1,
                Some(false) => tally.failed += 1,
                None => tally.undecided += 1,
            },
        }
    }
    tally
}

pub fn active_participant_ids(state: &ReviewStateFile) -> Vec<String> {
    state
        .participants
        .iter()
        .filter(|id| !state.abandoned_participants.contains(id))
        .cloned()
        .collect()
}

pub fn all_reviewers_passed(opinions: &[ReviewOpinionWire], state: &ReviewStateFile) -> bool {
    let active = active_participant_ids(state);
    if active.is_empty() {
        return false;
    }
    active.iter().all(|participant_id| {
        opinions
            .iter()
            .find(|o| o.employee_id == *participant_id)
            .is_some_and(|o| {
                o.status == OpinionItemStatus::Completed && o.passed == Some(true)
            })
    })
}

pub fn derive_review_conclusion(
    opinions: &[ReviewOpinionWire],
    state: &ReviewStateFile,
) -> Option<ReviewConclusion> {
    let active = active_participant_ids(state);
    if active.is_empty() {
        return None;
    }
    if !active.iter().all(|participant_id| {
        opinions
            .iter()
            .find(|o| o.employee_id == *participant_id)
            .is_some_and(|o| o.status == OpinionItemStatus::Completed)
    }) {
        return None;
    }
    if all_reviewers_passed(opinions, state) {
        Some(ReviewConclusion::Adopt)
    } else {
        Some(ReviewConclusion::Supplement)
    }
}

pub fn opinion_item_status(
    state: &ReviewStateFile,
    employee_id: &str,
    has_nonempty_opinion: bool,
) -> OpinionItemStatus {
    if state.abandoned_participants.iter().any(|id| id == employee_id) {
        return OpinionItemStatus::Abandoned;
    }
    if state.status == ReviewStatus::InProgress
        && state.current_reviewer_id.as_deref() == Some(employee_id)
    {
        if state.current_task == Some(ReviewTask::Revise) {
            return OpinionItemStatus::Revising;
        }
        return OpinionItemStatus::InProgress;
    }
    if has_nonempty_opinion {
        return OpinionItemStatus::Completed;
    }
    OpinionItemStatus::Pending
}

fn read_opinion_content(workspace: &Path, requirement_id: &str, employee_id: &str) -> Option<String> {
    let path = opinions_root(workspace, requirement_id).join(format!("{employee_id}.md"));
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).unwrap_or_default();
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

fn load_opinions(
    workspace: &Path,
    requirement_id: &str,
    state: &ReviewStateFile,
    employees: &[EmployeeRecord],
) -> Vec<ReviewOpinionWire> {
    let rules = load_work_rules(workspace).ok();
    let employee_by_id: std::collections::HashMap<&str, &EmployeeRecord> =
        employees.iter().map(|e| (e.id.as_str(), e)).collect();

    state
        .participants
        .iter()
        .map(|participant_id| {
            let emp = employee_by_id.get(participant_id.as_str());
            let employee_id = participant_id.clone();
            let employee_name = emp
                .map(|e| e.name.clone())
                .unwrap_or_else(|| participant_id.clone());
            let role = emp
                .map(|e| e.role.clone())
                .unwrap_or_else(|| "—".to_string());
            let role_key = emp
                .and_then(|e| rules.as_ref().and_then(|r| resolve_role_key(r, &e.role)));
            let content = read_opinion_content(workspace, requirement_id, &employee_id);
            let has_file = content.is_some();
            let status = opinion_item_status(state, &employee_id, has_file);
            let passed = content.as_deref().and_then(parse_opinion_passed);
            ReviewOpinionWire {
                employee_id,
                employee_name,
                role,
                role_key,
                status,
                passed,
                content,
            }
        })
        .collect()
}

pub fn load_review_wire(workspace: &Path, requirement_id: &str) -> anyhow::Result<RequirementReviewWire> {
    let id = normalize_requirement_id(requirement_id)?;
    let state = load_state(workspace, &id)?;
    let employees = list_employee_records(workspace)?;
    let opinions = load_opinions(workspace, &id, &state, &employees);
    let summary = summary_path(workspace, &id)
        .exists()
        .then(|| fs::read_to_string(summary_path(workspace, &id)).ok())
        .flatten();
    let tally = review_tally(&opinions);
    let abandoned_count = opinions
        .iter()
        .filter(|o| o.status == OpinionItemStatus::Abandoned)
        .count() as u32;
    let overall_passed =
        state.status == ReviewStatus::Completed && all_reviewers_passed(&opinions, &state);
    Ok(RequirementReviewWire {
        requirement_id: id,
        status: state.status,
        started_at_ms: state.started_at_ms,
        completed_at_ms: state.completed_at_ms,
        conclusion: state.conclusion,
        participants: state.participants,
        opinions,
        summary,
        passed_count: tally.passed,
        failed_count: tally.failed,
        pending_count: tally.pending,
        undecided_count: tally.undecided,
        abandoned_count,
        overall_passed,
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
3. Use Markdown with sections: Summary, Findings, Risks, Recommendation.
4. End the opinion with **exactly one** line: `Recommendation: approve` or `Recommendation: needs change`.
5. Reply briefly confirming the file you wrote.

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

fn build_revision_messages(
    rules: &WorkRulesFile,
    employee: &EmployeeRecord,
    role_key: Option<&str>,
    requirement_id: &str,
    requirement_title: &str,
    prior_opinion: &str,
) -> Vec<ToolChatMessage> {
    let role_label = role_key
        .and_then(|k| rules.roles.get(k))
        .map(|r| r.display_name.as_str())
        .unwrap_or(employee.role.as_str());
    let system = format!(
        r#"You are **{role_label}** ({employee_name}) revising requirement `{requirement_id}` after your review did NOT pass.

## Working directory
Requirement package directory. Requirement body: `{REQUIREMENT_FILE}`. Your prior opinion: `review/opinions/{employee_id}.md`.

## Task
1. Read `{REQUIREMENT_FILE}` and your prior opinion file.
2. Edit `{REQUIREMENT_FILE}` to address every blocking issue you raised (you must fix the requirement yourself).
3. Overwrite `review/opinions/{employee_id}.md` with an updated review of the revised requirement.
4. End the updated opinion with **exactly one** line: `Recommendation: approve` or `Recommendation: needs change` (use `approve` only if you would now pass).
5. Reply briefly listing what you changed in the requirement.

Do not only describe intent — perform the file edits."#,
        role_label = role_label,
        employee_name = employee.name,
        requirement_id = requirement_id,
        REQUIREMENT_FILE = REQUIREMENT_FILE,
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
                "Revise requirement **{requirement_title}** (`{requirement_id}`) until you can approve it.\n\n## Your prior opinion\n\n{prior_opinion}"
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
        let body = op.content.as_deref().unwrap_or("(no opinion body)");
        catalog.push_str(&format!(
            "### {} ({}) — {}\n{}\n\n",
            op.employee_name, op.role, op.employee_id, body
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
     - `adopt` — every reviewer passed (all `Recommendation: approve`)
     - `supplement` — at least one reviewer did not pass
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

fn delete_opinion_file(workspace: &Path, requirement_id: &str, employee_id: &str) -> anyhow::Result<()> {
    let path = opinions_root(workspace, requirement_id).join(format!("{employee_id}.md"));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn reopen_review_if_completed(workspace: &Path, state: &mut ReviewStateFile) -> anyhow::Result<()> {
    if state.status != ReviewStatus::Completed {
        return Ok(());
    }
    state.status = ReviewStatus::InProgress;
    state.completed_at_ms = None;
    state.conclusion = None;
    state.current_reviewer_id = None;
    state.current_task = None;
    save_state(workspace, state)?;
    set_requirement_phase(workspace, &state.requirement_id, RequirementPhase::Review)?;
    Ok(())
}

fn write_manual_opinion(
    workspace: &Path,
    requirement_id: &str,
    employee_id: &str,
    passed: bool,
) -> anyhow::Result<String> {
    fs::create_dir_all(opinions_root(workspace, requirement_id))?;
    let recommendation = if passed {
        "Recommendation: approve"
    } else {
        "Recommendation: needs change"
    };
    let body = format!(
        "## Manual review (user)\n\nRecorded at {ts} ms.\n\n{recommendation}\n",
        ts = now_ms(),
        recommendation = recommendation
    );
    let path = opinions_root(workspace, requirement_id).join(format!("{employee_id}.md"));
    fs::write(&path, &body)?;
    Ok(body)
}

fn find_employee<'a>(employees: &'a [EmployeeRecord], employee_id: &str) -> anyhow::Result<&'a EmployeeRecord> {
    employees
        .iter()
        .find(|e| e.id == employee_id)
        .ok_or_else(|| anyhow::anyhow!("review_opinion_not_participant"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpinionUserAction {
    Rerun,
    Pass,
    Fail,
    Abandon,
}

pub fn parse_opinion_user_action(action: &str) -> Option<OpinionUserAction> {
    match action {
        "rerun" => Some(OpinionUserAction::Rerun),
        "pass" => Some(OpinionUserAction::Pass),
        "fail" => Some(OpinionUserAction::Fail),
        "abandon" => Some(OpinionUserAction::Abandon),
        _ => None,
    }
}

pub fn apply_opinion_user_action(
    workspace: &Path,
    tools: &ToolManager,
    requirement_id: &str,
    employee_id: &str,
    action: OpinionUserAction,
) -> anyhow::Result<RequirementReviewWire> {
    let id = normalize_requirement_id(requirement_id)?;
    let mut state = if state_path(workspace, &id).exists() {
        load_state(workspace, &id)?
    } else {
        start_review(workspace, &id)?
    };

    if !state.participants.iter().any(|p| p == employee_id) {
        anyhow::bail!("review_opinion_not_participant");
    }
    if state.current_reviewer_id.is_some() {
        anyhow::bail!("review_opinion_busy");
    }

    reopen_review_if_completed(workspace, &mut state)?;
    state = load_state(workspace, &id)?;

    let detail = load_requirement_detail(workspace, &id)?;
    let employees = list_employee_records(workspace)?;
    let employee = find_employee(&employees, employee_id)?;

    match action {
        OpinionUserAction::Rerun => {
            state.abandoned_participants.retain(|p| p != employee_id);
            save_state(workspace, &state)?;
            delete_opinion_file(workspace, &id, employee_id)?;
            let workdir = requirement_workdir(workspace, &id);
            let rules = load_work_rules(workspace)?;
            let mut state = load_state(workspace, &id)?;
            run_employee_review(
                workspace,
                tools,
                &rules,
                &mut state,
                employee,
                &id,
                &detail.title,
                &detail.content,
                &workdir,
            )?;
        }
        OpinionUserAction::Pass => {
            state.abandoned_participants.retain(|p| p != employee_id);
            save_state(workspace, &state)?;
            write_manual_opinion(workspace, &id, employee_id, true)?;
        }
        OpinionUserAction::Fail => {
            state.abandoned_participants.retain(|p| p != employee_id);
            save_state(workspace, &state)?;
            write_manual_opinion(workspace, &id, employee_id, false)?;
        }
        OpinionUserAction::Abandon => {
            if !state.abandoned_participants.iter().any(|p| p == employee_id) {
                state.abandoned_participants.push(employee_id.to_string());
            }
            save_state(workspace, &state)?;
            delete_opinion_file(workspace, &id, employee_id)?;
        }
    }

    load_review_wire(workspace, &id)
}

fn run_employee_review(
    workspace: &Path,
    tools: &ToolManager,
    rules: &WorkRulesFile,
    state: &mut ReviewStateFile,
    employee: &EmployeeRecord,
    requirement_id: &str,
    title: &str,
    content: &str,
    workdir: &Path,
) -> anyhow::Result<String> {
    state.current_reviewer_id = Some(employee.id.clone());
    state.current_task = Some(ReviewTask::Opinion);
    save_state(workspace, state)?;

    let role_key = resolve_role_key(rules, &employee.role);
    let messages = build_reviewer_messages(
        rules,
        employee,
        role_key.as_deref(),
        requirement_id,
        title,
        content,
    );
    let (_instance, result) = tools.execute_code_chat(workdir, &messages)?;
    let opinion = ensure_opinion_file(workspace, requirement_id, &employee.id, &result.output)?;

    state.current_reviewer_id = None;
    state.current_task = None;
    save_state(workspace, state)?;

    let section = format!("Review opinion — {}", now_ms());
    append_employee_memory(
        workspace,
        &employee.id,
        &section,
        &memory_section_review(requirement_id, title, &opinion),
    )?;
    Ok(opinion)
}

fn run_employee_revision(
    workspace: &Path,
    tools: &ToolManager,
    rules: &WorkRulesFile,
    state: &mut ReviewStateFile,
    employee: &EmployeeRecord,
    requirement_id: &str,
    title: &str,
    prior_opinion: &str,
    workdir: &Path,
) -> anyhow::Result<()> {
    state.current_reviewer_id = Some(employee.id.clone());
    state.current_task = Some(ReviewTask::Revise);
    save_state(workspace, state)?;

    let role_key = resolve_role_key(rules, &employee.role);
    let messages = build_revision_messages(
        rules,
        employee,
        role_key.as_deref(),
        requirement_id,
        title,
        prior_opinion,
    );
    let (_instance, result) = tools.execute_code_chat(workdir, &messages)?;
    let _ = ensure_opinion_file(workspace, requirement_id, &employee.id, &result.output)?;

    state.current_reviewer_id = None;
    state.current_task = None;
    save_state(workspace, state)?;

    let section = format!("Requirement revision after review — {}", now_ms());
    append_employee_memory(
        workspace,
        &employee.id,
        &section,
        &memory_section_review(requirement_id, title, &result.output),
    )?;
    Ok(())
}

fn employee_needs_initial_review(workspace: &Path, requirement_id: &str, employee_id: &str) -> bool {
    match read_opinion_content(workspace, requirement_id, employee_id) {
        None => true,
        Some(content) => parse_opinion_passed(&content) != Some(true),
    }
}

fn failed_participant_ids(opinions: &[ReviewOpinionWire]) -> Vec<String> {
    opinions
        .iter()
        .filter(|o| o.passed == Some(false))
        .map(|o| o.employee_id.clone())
        .collect()
}

pub fn run_requirement_review(
    workspace: &Path,
    tools: &ToolManager,
    requirement_id: &str,
) -> anyhow::Result<RequirementReviewWire> {
    let id = normalize_requirement_id(requirement_id)?;
    let mut state = load_state(workspace, &id)?;
    if state.status == ReviewStatus::Completed {
        anyhow::bail!("review_already_completed");
    }
    let rules = load_work_rules(workspace)?;
    let mut detail = load_requirement_detail(workspace, &id)?;
    let workdir = requirement_workdir(workspace, &id);
    let employees = list_employee_records(workspace)?;

    for employee in &employees {
        if !employee_needs_initial_review(workspace, &id, &employee.id) {
            continue;
        }
        run_employee_review(
            workspace,
            tools,
            &rules,
            &mut state,
            employee,
            &id,
            &detail.title,
            &detail.content,
            &workdir,
        )?;
    }

    for _round in 0..MAX_REVISION_ROUNDS {
        state = load_state(workspace, &id)?;
        let opinions = load_opinions(workspace, &id, &state, &employees);
        if all_reviewers_passed(&opinions, &state) {
            break;
        }
        let failed_ids = failed_participant_ids(&opinions);
        if failed_ids.is_empty() {
            break;
        }
        detail = load_requirement_detail(workspace, &id)?;
        for employee in &employees {
            if !failed_ids.iter().any(|fid| fid == &employee.id) {
                continue;
            }
            let prior = read_opinion_content(workspace, &id, &employee.id)
                .unwrap_or_else(|| "(no prior opinion)".to_string());
            run_employee_revision(
                workspace,
                tools,
                &rules,
                &mut state,
                employee,
                &id,
                &detail.title,
                &prior,
                &workdir,
            )?;
        }
    }

    state = load_state(workspace, &id)?;
    detail = load_requirement_detail(workspace, &id)?;
    let opinions = load_opinions(workspace, &id, &state, &employees);
    let summary_messages = build_summary_messages(&id, &detail.title, &opinions);
    let (_summary_inst, summary_result) = tools.execute_code_chat(&workdir, &summary_messages)?;
    let summary_text = if summary_path(workspace, &id).exists() {
        fs::read_to_string(summary_path(workspace, &id))?
    } else {
        let t = summary_result.output.clone();
        fs::write(summary_path(workspace, &id), &t)?;
        t
    };

    let conclusion = derive_review_conclusion(&opinions, &state)
        .or_else(|| parse_conclusion(&summary_text))
        .or_else(|| parse_conclusion(&summary_result.output))
        .unwrap_or(ReviewConclusion::Supplement);

    let next_phase = match conclusion {
        ReviewConclusion::Adopt => RequirementPhase::Confirm,
        ReviewConclusion::Supplement => RequirementPhase::Collection,
    };
    set_requirement_phase(workspace, &id, next_phase)?;

    state = load_state(workspace, &id)?;
    state.status = ReviewStatus::Completed;
    state.completed_at_ms = Some(now_ms());
    state.conclusion = Some(conclusion);
    state.current_reviewer_id = None;
    state.current_task = None;
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

/// Marks the review as passed and moves the requirement to the confirm phase (user override).
pub fn force_pass_review(workspace: &Path, requirement_id: &str) -> anyhow::Result<RequirementReviewWire> {
    let id = normalize_requirement_id(requirement_id)?;
    let detail = load_requirement_detail(workspace, &id)?;

    let mut state = if state_path(workspace, &id).exists() {
        load_state(workspace, &id)?
    } else {
        start_review(workspace, &id)?
    };

    fs::create_dir_all(review_root(workspace, &id))?;
    if !summary_path(workspace, &id).exists() {
        fs::write(
            summary_path(workspace, &id),
            format!(
                "# Review summary (force passed)\n\nRequirement **{}** (`{}`) was manually approved by the user.\n\nCONCLUSION: adopt\n",
                detail.title, id
            ),
        )?;
    }

    state.status = ReviewStatus::Completed;
    state.completed_at_ms = Some(now_ms());
    state.conclusion = Some(ReviewConclusion::Adopt);
    state.current_reviewer_id = None;
    state.current_task = None;
    save_state(workspace, &state)?;

    set_requirement_phase(workspace, &id, RequirementPhase::Confirm)?;
    load_review_wire(workspace, &id)
}

pub async fn opinion_action_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath((id, employee_id, action)): AxumPath<(String, String, String)>,
) -> Result<Json<RequirementReviewWire>, (axum::http::StatusCode, String)> {
    let Some(workspace) = workspace_root(&state) else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let parsed = parse_opinion_user_action(&action).ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            i18n::msg(&headers, "review_opinion_invalid_action"),
        )
    })?;

    if parsed == OpinionUserAction::Rerun {
        let tools = state.tools.read().expect("tools lock poisoned").clone();
        let workspace_path = workspace.clone();
        let req_id = normalize_requirement_id(&id).map_err(map_review_err(&headers))?;
        let emp_id = employee_id.clone();

        let mut prep_state = if state_path(&workspace, &req_id).exists() {
            load_state(&workspace, &req_id).map_err(map_review_err(&headers))?
        } else {
            start_review(&workspace, &req_id).map_err(map_review_err(&headers))?
        };

        if prep_state.current_reviewer_id.is_some() {
            return load_review_wire(&workspace, &req_id)
                .map(Json)
                .map_err(map_review_err(&headers));
        }

        reopen_review_if_completed(&workspace, &mut prep_state).map_err(map_review_err(&headers))?;
        prep_state = load_state(&workspace, &req_id).map_err(map_review_err(&headers))?;
        prep_state.abandoned_participants.retain(|p| p != &emp_id);
        save_state(&workspace, &prep_state).map_err(map_review_err(&headers))?;
        delete_opinion_file(&workspace, &req_id, &emp_id).map_err(map_review_err(&headers))?;

        let run_req_id = req_id.clone();
        tokio::task::spawn_blocking(move || {
            let Ok(mut st) = load_state(&workspace_path, &run_req_id) else {
                return;
            };
            let Ok(detail) = load_requirement_detail(&workspace_path, &run_req_id) else {
                return;
            };
            let Ok(employees) = list_employee_records(&workspace_path) else {
                return;
            };
            let Some(employee) = employees.iter().find(|e| e.id == emp_id) else {
                return;
            };
            let Ok(rules) = load_work_rules(&workspace_path) else {
                return;
            };
            let workdir = requirement_workdir(&workspace_path, &run_req_id);
            let _ = run_employee_review(
                &workspace_path,
                &tools,
                &rules,
                &mut st,
                employee,
                &run_req_id,
                &detail.title,
                &detail.content,
                &workdir,
            );
        });

        return load_review_wire(&workspace, &req_id)
            .map(Json)
            .map_err(map_review_err(&headers));
    }

    let tools = state.tools.read().expect("tools lock poisoned").clone();
    apply_opinion_user_action(&workspace, &tools, &id, &employee_id, parsed)
        .map(Json)
        .map_err(map_review_err(&headers))
}

pub async fn force_pass_review_handler(
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
    force_pass_review(&workspace, &id)
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
    let req_id = normalize_requirement_id(&id).map_err(map_review_err(&headers))?;

    if let Ok(existing) = load_state(&workspace, &req_id) {
        if existing.status == ReviewStatus::Completed {
            return Err(map_review_err(&headers)(anyhow::anyhow!("review_already_completed")));
        }
        if existing.status == ReviewStatus::InProgress && existing.current_reviewer_id.is_some() {
            return load_review_wire(&workspace, &req_id)
                .map(Json)
                .map_err(map_review_err(&headers));
        }
    } else {
        start_review(&workspace, &req_id).map_err(map_review_err(&headers))?;
    }

    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workspace_path = workspace.clone();
    let run_id = req_id.clone();
    tokio::task::spawn_blocking(move || run_requirement_review(&workspace_path, &tools, &run_id));

    load_review_wire(&workspace, &req_id)
        .map(Json)
        .map_err(map_review_err(&headers))
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
            "review_opinion_busy",
            "review_opinion_not_participant",
            "review_opinion_invalid_action",
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

    #[test]
    fn parses_opinion_passed_from_recommendation() {
        assert_eq!(
            parse_opinion_passed("## Recommendation\napprove"),
            Some(true)
        );
        assert_eq!(
            parse_opinion_passed("Recommendation: needs change"),
            Some(false)
        );
    }

    #[test]
    fn opinion_item_status_reflects_current_reviewer() {
        let state = ReviewStateFile {
            requirement_id: "r1".to_string(),
            status: ReviewStatus::InProgress,
            started_at_ms: 0,
            completed_at_ms: None,
            conclusion: None,
            participants: vec!["e1".into(), "e2".into()],
            current_reviewer_id: Some("e2".into()),
            current_task: Some(ReviewTask::Opinion),
        };
        assert_eq!(
            opinion_item_status(&state, "e1", true),
            OpinionItemStatus::Completed
        );
        assert_eq!(
            opinion_item_status(&state, "e2", false),
            OpinionItemStatus::InProgress
        );
        assert_eq!(
            opinion_item_status(&state, "e1", false),
            OpinionItemStatus::Pending
        );
    }

    #[test]
    fn opinion_item_status_revising_when_adjusting_requirement() {
        let state = ReviewStateFile {
            requirement_id: "r1".to_string(),
            status: ReviewStatus::InProgress,
            started_at_ms: 0,
            completed_at_ms: None,
            conclusion: None,
            participants: vec!["e1".into()],
            current_reviewer_id: Some("e1".into()),
            current_task: Some(ReviewTask::Revise),
        };
        assert_eq!(
            opinion_item_status(&state, "e1", true),
            OpinionItemStatus::Revising
        );
    }

    #[test]
    fn review_tally_counts_pass_and_fail() {
        let opinions = vec![
            ReviewOpinionWire {
                employee_id: "a".into(),
                employee_name: "A".into(),
                role: "r".into(),
                role_key: None,
                status: OpinionItemStatus::Completed,
                passed: Some(true),
                content: None,
            },
            ReviewOpinionWire {
                employee_id: "b".into(),
                employee_name: "B".into(),
                role: "r".into(),
                role_key: None,
                status: OpinionItemStatus::Completed,
                passed: Some(false),
                content: None,
            },
        ];
        let tally = review_tally(&opinions);
        assert_eq!(tally.passed, 1);
        assert_eq!(tally.failed, 1);
    }

    fn sample_state(participants: Vec<&str>) -> ReviewStateFile {
        ReviewStateFile {
            requirement_id: "r1".into(),
            status: ReviewStatus::InProgress,
            started_at_ms: 0,
            completed_at_ms: None,
            conclusion: None,
            participants: participants.into_iter().map(String::from).collect(),
            current_reviewer_id: None,
            current_task: None,
            abandoned_participants: Vec::new(),
        }
    }

    #[test]
    fn overall_pass_requires_every_active_reviewer_approved() {
        let state = sample_state(vec!["a"]);
        let ok = vec![ReviewOpinionWire {
            employee_id: "a".into(),
            employee_name: "A".into(),
            role: "r".into(),
            role_key: None,
            status: OpinionItemStatus::Completed,
            passed: Some(true),
            content: None,
        }];
        assert!(all_reviewers_passed(&ok, &state));
        assert_eq!(
            derive_review_conclusion(&ok, &state),
            Some(ReviewConclusion::Adopt)
        );

        let state2 = sample_state(vec!["a", "b"]);
        let mixed = vec![
            ReviewOpinionWire {
                employee_id: "a".into(),
                employee_name: "A".into(),
                role: "r".into(),
                role_key: None,
                status: OpinionItemStatus::Completed,
                passed: Some(true),
                content: None,
            },
            ReviewOpinionWire {
                employee_id: "b".into(),
                employee_name: "B".into(),
                role: "r".into(),
                role_key: None,
                status: OpinionItemStatus::Completed,
                passed: Some(false),
                content: None,
            },
        ];
        assert!(!all_reviewers_passed(&mixed, &state2));
        assert_eq!(
            derive_review_conclusion(&mixed, &state2),
            Some(ReviewConclusion::Supplement)
        );
    }

    #[test]
    fn abandoned_participant_does_not_block_overall_pass() {
        let mut state = sample_state(vec!["a", "b"]);
        state.abandoned_participants.push("b".into());
        let opinions = vec![ReviewOpinionWire {
            employee_id: "a".into(),
            employee_name: "A".into(),
            role: "r".into(),
            role_key: None,
            status: OpinionItemStatus::Completed,
            passed: Some(true),
            content: None,
        }];
        assert!(all_reviewers_passed(&opinions, &state));
    }
}
