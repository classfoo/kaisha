use crate::{
    autonomy_trigger::{clear_pending_autonomy, list_pending_autonomy_employees, mark_employee_for_autonomy},
    employee::{list_employee_records, EmployeeRecord},
    employee_todo::{
        add_todo, count_incomplete_todos, load_todos, mark_todo_status, next_pending_todo,
        save_todos, TodoSource, TodoStatus,
    },
    git::{list_repos, repo_dir, MAIN_REPO_ID},
    requirement::{
        ensure_requirements_root, format_requirement_md, list_requirement_summaries,
        load_requirement_detail, phase_in_progress, RequirementMeta, RequirementPhase,
        RequirementSummary, REQUIREMENT_FILE,
    },
    requirement_development::{
        add_development_task, development_requirements_need_planning,
        format_development_requirement_context, list_development_requirements_needing_tasks,
    },
    tasks::{
        autonomy_execute_content, autonomy_explore_content, AgentTaskRecord, CodeAgentTaskParams,
        TaskKind, TaskRunner, TaskStatus, TaskStore,
    },
    tools::driver::ToolChatMessage,
    work_rules::{duty_for_phase, load_work_rules, resolve_role_key},
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
    sync::{Arc, Mutex, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Notify;

pub const AUTONOMY_INTERVAL_SECS: u64 = 900;
pub const AUTONOMY_DEBOUNCE_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorationMode {
    RequirementPlanning,
    GitExploration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyTriggerKind {
    TaskCompleted,
    Scheduled,
    Manual,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutonomyStatusWire {
    pub enabled: bool,
    pub interval_secs: u64,
    pub pending_employees: Vec<String>,
    pub last_tick_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParsedAutonomyTodo {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub requirement_id: Option<String>,
    #[serde(default)]
    pub requirement_phase: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParsedNewRequirement {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub phase: Option<String>,
  #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParsedAutonomyPlan {
    #[serde(default, rename = "mode")]
    pub _mode: Option<String>,
    #[serde(default)]
    pub todos: Vec<ParsedAutonomyTodo>,
    #[serde(default)]
    pub new_requirements: Vec<ParsedNewRequirement>,
}

pub fn has_participatable_requirements(
    summaries: &[crate::requirement::RequirementSummary],
) -> bool {
    summaries.iter().any(|item| phase_in_progress(&item.phase))
}

pub fn select_exploration_mode(
    summaries: &[crate::requirement::RequirementSummary],
) -> ExplorationMode {
    if has_participatable_requirements(summaries) {
        ExplorationMode::RequirementPlanning
    } else {
        ExplorationMode::GitExploration
    }
}

pub fn is_employee_busy(tasks: &[AgentTaskRecord], employee_id: &str) -> bool {
    is_employee_busy_excluding(tasks, employee_id, None)
}

pub fn is_employee_busy_excluding(
    tasks: &[AgentTaskRecord],
    employee_id: &str,
    exclude_task_id: Option<&str>,
) -> bool {
    tasks.iter().any(|task| {
        if exclude_task_id == Some(task.id.as_str()) {
            return false;
        }
        task.executor_id.as_deref() == Some(employee_id)
            && matches!(task.status, TaskStatus::Pending | TaskStatus::Running)
    })
}

pub fn should_run_autonomy(
    incomplete_todos: usize,
    employee_busy: bool,
    last_autonomy_run_ms: Option<u64>,
    now_ms: u64,
    trigger: AutonomyTriggerKind,
    development_planning_needed: bool,
) -> bool {
    should_start_autonomy_exploration(
        incomplete_todos,
        employee_busy,
        last_autonomy_run_ms,
        now_ms,
        trigger,
        false,
        development_planning_needed,
    )
}

pub fn should_start_autonomy_exploration(
    incomplete_todos: usize,
    employee_busy: bool,
    last_autonomy_run_ms: Option<u64>,
    now_ms: u64,
    trigger: AutonomyTriggerKind,
    force: bool,
    development_planning_needed: bool,
) -> bool {
    if employee_busy {
        return false;
    }
    if force {
        return true;
    }
    match trigger {
        AutonomyTriggerKind::TaskCompleted | AutonomyTriggerKind::Manual => {
            if let Some(last) = last_autonomy_run_ms {
                return now_ms.saturating_sub(last) >= 5_000;
            }
            true
        }
        AutonomyTriggerKind::Scheduled => {
            if incomplete_todos > 0 && !development_planning_needed {
                return false;
            }
            if let Some(last) = last_autonomy_run_ms {
                return now_ms.saturating_sub(last) >= AUTONOMY_DEBOUNCE_MS;
            }
            true
        }
    }
}

pub fn should_execute_next_todo(incomplete_todos: usize, employee_busy: bool) -> bool {
    !employee_busy && incomplete_todos > 0
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn parse_autonomy_plan_output(output: &str) -> Option<ParsedAutonomyPlan> {
    let candidates = autonomy_plan_json_candidates(output);
    for candidate in candidates {
        if let Ok(plan) = serde_json::from_str::<ParsedAutonomyPlan>(&candidate) {
            if !plan.todos.is_empty() || !plan.new_requirements.is_empty() {
                return Some(plan);
            }
        }
    }
    None
}

fn autonomy_plan_json_candidates(output: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let trimmed = output.trim();
    if trimmed.starts_with('{') {
        candidates.push(trimmed.to_string());
    }
    for line in output.lines() {
        let line = line.trim();
        if line.starts_with('{') {
            candidates.push(line.to_string());
        }
    }
    let mut rest = output;
    while let Some(start) = rest.find("```") {
        let after_fence = &rest[start + 3..];
        let body = after_fence
            .strip_prefix("json")
            .or_else(|| after_fence.strip_prefix("JSON"))
            .unwrap_or(after_fence);
        if let Some(end) = body.find("```") {
            let block = body[..end].trim();
            if block.starts_with('{') {
                candidates.push(block.to_string());
            }
            rest = &body[end + 3..];
        } else {
            break;
        }
    }
    candidates
}

pub fn has_development_phase_requirements(
    summaries: &[RequirementSummary],
) -> bool {
    summaries
        .iter()
        .any(|item| item.phase == RequirementPhase::Development)
}

fn format_requirement_context(
    workspace: &Path,
    employee: &EmployeeRecord,
) -> anyhow::Result<String> {
    let rules = load_work_rules(workspace)?;
    let role_key = resolve_role_key(&rules, &employee.role).unwrap_or_else(|| "engineering".to_string());
    let summaries = list_requirement_summaries(workspace)?;
    let mut out = String::new();
    out.push_str(&format!(
        "Employee: {} (id: {}, role: {}, resolved_role: {})\n\n",
        employee.name, employee.id, employee.role, role_key
    ));
    if summaries.is_empty() {
        out.push_str("No requirements exist yet.\n");
        return Ok(out);
    }
    for item in &summaries {
        if !phase_in_progress(&item.phase) {
            continue;
        }
        let phase = item.phase.as_str();
        let duty = duty_for_phase(&rules, &role_key, phase);
        if item.phase == RequirementPhase::Development {
            out.push_str(&format_development_requirement_context(workspace, item)?);
            out.push_str(&format!("  role_duty: {duty}\n\n"));
            continue;
        }
        out.push_str(&format!(
            "- id: {}\n  title: {}\n  phase: {}\n  role_duty: {}\n  file: requirements/{}/{}\n",
            item.id, item.title, phase, duty, item.id, REQUIREMENT_FILE
        ));
        if let Ok(detail) = load_requirement_detail(workspace, &item.id) {
            let excerpt: String = detail.content.chars().take(480).collect();
            if !excerpt.trim().is_empty() {
                out.push_str("  content_excerpt: |\n");
                for line in excerpt.lines() {
                    out.push_str(&format!("    {line}\n"));
                }
            }
        }
        out.push('\n');
    }
    Ok(out)
}

fn format_git_context(workspace: &Path) -> anyhow::Result<String> {
    let repos = list_repos(workspace)?;
    let mut out = String::new();
    if repos.is_empty() {
        out.push_str("No git repositories registered.\n");
        return Ok(out);
    }
    let main = repos
        .iter()
        .find(|r| r.is_main || r.id == MAIN_REPO_ID)
        .or_else(|| repos.first());
    if let Some(repo) = main {
        let dir = repo_dir(workspace, &repo.id);
        out.push_str(&format!(
            "Primary repo: id={}, name={}, path={}\n",
            repo.id,
            repo.name,
            dir.display()
        ));
        if dir.exists() {
            out.push_str("Browse this repository to find UX and technical improvement opportunities.\n");
        } else {
            out.push_str("Repository directory does not exist yet.\n");
        }
    }
    Ok(out)
}

fn build_explore_messages(
    workspace: &Path,
    employee: &EmployeeRecord,
    mode: ExplorationMode,
) -> anyhow::Result<Vec<ToolChatMessage>> {
    let requirement_ctx = format_requirement_context(workspace, employee)?;
    let git_ctx = format_git_context(workspace)?;
    let mode_instructions = match mode {
        ExplorationMode::RequirementPlanning if has_development_phase_requirements(
            &list_requirement_summaries(workspace)?,
        ) => {
            r#"Requirements in the `development` phase need implementation breakdown now.

Rules:
1. Read each development-phase requirement's content excerpt and existing development tasks.
2. Break the requirement into concrete implementation todos for this employee's role duty.
3. Every todo MUST include `requirement_id` and `requirement_phase: "development"`.
4. Each todo becomes both an employee todo and a requirement development task.
5. Prefer 2-5 focused tasks that can be executed in separate sessions.
6. Do not duplicate todos already listed in the employee todo file or development task list."#
        }
        ExplorationMode::RequirementPlanning => {
            r#"Review in-progress requirements and create actionable todos aligned with each requirement's phase and the employee's role duty.

Rules:
1. Prefer updating work on existing in-progress requirements.
2. Each todo must be concrete and executable in one focused session.
3. Link todos to requirement_id and requirement_phase when applicable.
4. Do not duplicate todos already listed in the employee todo file."#
        }
        ExplorationMode::GitExploration => {
            r#"No in-progress requirements are available for this employee to join. Browse the product git repository, analyze UX and technical issues, and propose improvement work.

Rules:
1. Inspect code, docs, and recent structure in the repo working directory when available.
2. You may propose new experience requirements or technical refactor requirements.
3. Put executable todos in `todos` and new requirement proposals in `new_requirements`.
4. New requirements must use phase `collection` unless clearly ready for review."#
        }
    };

    let existing = load_todos(workspace, &employee.id)?;
    let existing_todos = if existing.items.is_empty() {
        "(none)\n".to_string()
    } else {
        existing
            .items
            .iter()
            .map(|item| {
                format!(
                    "- [{}] {} ({:?})",
                    format!("{:?}", item.status).to_lowercase(),
                    item.title,
                    item.source
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };

    let system = format!(
        r#"You are an employee autonomy planner for a software team.

{mode_instructions}

Respond with ONLY one JSON object:
{{
  "mode": "requirement_planning" | "git_exploration",
  "todos": [
    {{
      "title": "short actionable title",
      "description": "what to do and expected outcome",
      "requirement_id": "optional",
      "requirement_phase": "optional"
    }}
  ],
  "new_requirements": [
    {{
      "id": "kebab-case-id",
      "title": "requirement title",
      "phase": "collection",
      "content": "markdown body"
    }}
  ]
}}

## Requirement context
{requirement_ctx}

## Git context
{git_ctx}

## Existing employee todos
{existing_todos}
"#
    );

    Ok(vec![
        ToolChatMessage {
            role: "system".into(),
            content: system,
        },
        ToolChatMessage {
            role: "user".into(),
            content: "Plan the next actionable todos for this employee now.".into(),
        },
    ])
}

fn build_execute_messages(
    workspace: &Path,
    employee: &EmployeeRecord,
    todo: &crate::employee_todo::EmployeeTodoItem,
) -> anyhow::Result<Vec<ToolChatMessage>> {
    let mut ctx = format!(
        "Employee: {} ({})\nTodo: {}\nDescription: {}\n",
        employee.name, employee.role, todo.title, todo.description
    );
    if let Some(ref req_id) = todo.requirement_id {
        ctx.push_str(&format!("Requirement id: {req_id}\n"));
        if let Ok(detail) = load_requirement_detail(workspace, req_id) {
            let excerpt: String = detail.content.chars().take(1200).collect();
            ctx.push_str(&format!("\nRequirement excerpt:\n{excerpt}\n"));
        }
    }
    Ok(vec![
        ToolChatMessage {
            role: "system".into(),
            content: format!(
                r#"You are {name}, a {role} executing one assigned todo.

Working directory is the workspace root. Use file tools and git as needed.
When finished, summarize what you changed or learned.

{ctx}"#,
                name = employee.name,
                role = employee.role,
                ctx = ctx
            ),
        },
        ToolChatMessage {
            role: "user".into(),
            content: format!("Execute this todo now: {}", todo.title),
        },
    ])
}

fn parse_requirement_phase(raw: &str) -> RequirementPhase {
    match raw.trim().to_lowercase().as_str() {
        "review" => RequirementPhase::Review,
        "confirm" => RequirementPhase::Confirm,
        "development" => RequirementPhase::Development,
        "testing" => RequirementPhase::Testing,
        "release" => RequirementPhase::Release,
        _ => RequirementPhase::Collection,
    }
}

fn sync_todo_to_development_task(
    workspace: &Path,
    employee_id: &str,
    todo: &ParsedAutonomyTodo,
) -> anyhow::Result<bool> {
    let Some(requirement_id) = todo
        .requirement_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(false);
    };
    let detail = load_requirement_detail(workspace, requirement_id)?;
    let targets_development = detail.phase == RequirementPhase::Development
        || todo
            .requirement_phase
            .as_deref()
            .map(|phase| phase.eq_ignore_ascii_case("development"))
            .unwrap_or(false);
    if !targets_development {
        return Ok(false);
    }
    add_development_task(
        workspace,
        requirement_id,
        &todo.title,
        &todo.description,
        Some(employee_id),
    )
    .map_err(|err| anyhow::anyhow!(err))?;
    Ok(true)
}

fn seed_development_tasks_from_requirements(
    workspace: &Path,
    employee_id: &str,
) -> anyhow::Result<usize> {
    let mut added = 0;
    for item in list_development_requirements_needing_tasks(workspace)? {
        let detail = load_requirement_detail(workspace, &item.id)?;
        let title = format!("Implement {}", detail.title);
        let description = detail.content.chars().take(800).collect::<String>();
        add_todo(
            workspace,
            employee_id,
            &title,
            &description,
            TodoSource::Requirement,
            Some(&item.id),
            Some("development"),
        )?;
        add_development_task(
            workspace,
            &item.id,
            &title,
            &description,
            Some(employee_id),
        )
        .map_err(|err| anyhow::anyhow!(err))?;
        added += 1;
    }
    Ok(added)
}

pub fn apply_autonomy_plan(
    workspace: &Path,
    employee_id: &str,
    mode: ExplorationMode,
    plan: &ParsedAutonomyPlan,
) -> anyhow::Result<usize> {
    let source = match mode {
        ExplorationMode::RequirementPlanning => TodoSource::Requirement,
        ExplorationMode::GitExploration => TodoSource::GitExploration,
    };
    let req_root = ensure_requirements_root(workspace)?;
    for req in &plan.new_requirements {
        let id = req.id.as_deref().unwrap_or("").trim();
        if id.is_empty() {
            continue;
        }
        let dir = req_root.join(id);
        if dir.join(REQUIREMENT_FILE).exists() {
            continue;
        }
        fs::create_dir_all(&dir)?;
        let phase = req
            .phase
            .as_deref()
            .map(parse_requirement_phase)
            .unwrap_or(RequirementPhase::Collection);
        let ts = now_ms();
        let meta = RequirementMeta {
            id: id.to_string(),
            title: req.title.trim().to_string(),
            phase,
            confirm_status: None,
            created_at_ms: ts,
            updated_at_ms: ts,
        };
        let body = req.content.as_deref().unwrap_or("## Background\n\nAutonomy exploration proposal.\n");
        fs::write(dir.join(REQUIREMENT_FILE), format_requirement_md(&meta, body))?;
    }
    let mut added = 0;
    for todo in &plan.todos {
        if todo.title.trim().is_empty() {
            continue;
        }
        add_todo(
            workspace,
            employee_id,
            &todo.title,
            &todo.description,
            source,
            todo.requirement_id.as_deref(),
            todo.requirement_phase.as_deref(),
        )?;
        let _ = sync_todo_to_development_task(workspace, employee_id, todo)?;
        added += 1;
    }
    Ok(added)
}

pub fn run_autonomy_exploration(
    workspace: &Path,
    tools: &crate::tools::manager::ToolManager,
    employee: &EmployeeRecord,
    trigger: AutonomyTriggerKind,
    force: bool,
) -> anyhow::Result<()> {
    let summaries = list_requirement_summaries(workspace)?;
    let mode = select_exploration_mode(&summaries);
    let todos = load_todos(workspace, &employee.id)?;
    let tasks = TaskStore::new(workspace).list()?;
    let busy = is_employee_busy(&tasks, &employee.id);
    let development_planning_needed = development_requirements_need_planning(workspace);
    if !should_start_autonomy_exploration(
        count_incomplete_todos(&todos),
        busy,
        todos.last_autonomy_run_ms,
        now_ms(),
        trigger,
        force,
        development_planning_needed,
    ) {
        return Ok(());
    }

    let runner = TaskRunner::new(workspace);
    let messages = build_explore_messages(workspace, employee, mode)?;
    let workdir = workspace.to_path_buf();
    let (_task, _instance, result) = runner.run_code_chat(
        tools,
        CodeAgentTaskParams {
            kind: TaskKind::AutonomyExplore,
            content: autonomy_explore_content(&employee.id, &format!("{mode:?}")),
            workdir: workdir.clone(),
            messages,
            executor_id: Some(employee.id.clone()),
            parent_task_id: None,
            context: serde_json::json!({
                "employee_id": employee.id,
                "trigger": format!("{:?}", trigger).to_lowercase(),
                "mode": format!("{:?}", mode).to_lowercase(),
            }),
        },
    )?;

    let mut file = load_todos(workspace, &employee.id)?;
    file.last_autonomy_run_ms = Some(now_ms());

    if result.exit_code == 0 {
        let mut added = 0;
        if let Some(plan) = parse_autonomy_plan_output(&result.output) {
            added = apply_autonomy_plan(workspace, &employee.id, mode, &plan)?;
            file = load_todos(workspace, &employee.id)?;
            file.last_autonomy_run_ms = Some(now_ms());
        }
        if added == 0 && development_planning_needed {
            let _ = seed_development_tasks_from_requirements(workspace, &employee.id)?;
            file = load_todos(workspace, &employee.id)?;
            file.last_autonomy_run_ms = Some(now_ms());
        }
    }
    save_todos(workspace, &file)?;
    clear_pending_autonomy(workspace, &employee.id)?;
    Ok(())
}

pub fn execute_next_todo(
    workspace: &Path,
    tools: &crate::tools::manager::ToolManager,
    employee: &EmployeeRecord,
) -> anyhow::Result<bool> {
    let file = load_todos(workspace, &employee.id)?;
    let tasks = TaskStore::new(workspace).list()?;
    if !should_execute_next_todo(count_incomplete_todos(&file), is_employee_busy(&tasks, &employee.id)) {
        return Ok(false);
    }
    let Some(todo) = next_pending_todo(&file) else {
        return Ok(false);
    };
    let todo_id = todo.id.clone();
    mark_todo_status(workspace, &employee.id, &todo_id, TodoStatus::InProgress)?;

    let runner = TaskRunner::new(workspace);
    let messages = build_execute_messages(workspace, employee, todo)?;
    let result = runner.run_code_chat(
        tools,
        CodeAgentTaskParams {
            kind: TaskKind::AutonomyExecute,
            content: autonomy_execute_content(&employee.id, &todo.title),
            workdir: workspace.to_path_buf(),
            messages,
            executor_id: Some(employee.id.clone()),
            parent_task_id: None,
            context: serde_json::json!({
                "employee_id": employee.id,
                "todo_id": todo_id,
            }),
        },
    );

    match result {
        Ok((_task, _instance, exec)) => {
            let status = if exec.exit_code == 0 {
                TodoStatus::Completed
            } else {
                TodoStatus::Pending
            };
            mark_todo_status(workspace, &employee.id, &todo_id, status)?;
        }
        Err(_) => {
            mark_todo_status(workspace, &employee.id, &todo_id, TodoStatus::Pending)?;
        }
    }

    mark_employee_for_autonomy(workspace, &employee.id)?;
    Ok(true)
}

pub fn process_employee_autonomy(
    workspace: &Path,
    tools: &crate::tools::manager::ToolManager,
    employee: &EmployeeRecord,
    trigger: AutonomyTriggerKind,
) -> anyhow::Result<()> {
    let file = load_todos(workspace, &employee.id)?;
    let tasks = TaskStore::new(workspace).list()?;
    let busy = is_employee_busy(&tasks, &employee.id);
    if busy {
        return Ok(());
    }

    let development_planning_needed = development_requirements_need_planning(workspace);
    if matches!(trigger, AutonomyTriggerKind::TaskCompleted | AutonomyTriggerKind::Manual) {
        if should_run_autonomy(
            count_incomplete_todos(&file),
            false,
            file.last_autonomy_run_ms,
            now_ms(),
            trigger,
            development_planning_needed,
        ) {
            run_autonomy_exploration(workspace, tools, employee, trigger, false)?;
        }
    }

    let file = load_todos(workspace, &employee.id)?;
    let incomplete = count_incomplete_todos(&file);
    let development_planning_needed = development_requirements_need_planning(workspace);
    if should_execute_next_todo(incomplete, false) {
        execute_next_todo(workspace, tools, employee)?;
        return Ok(());
    }

    if matches!(trigger, AutonomyTriggerKind::Scheduled)
        && should_run_autonomy(
            incomplete,
            false,
            file.last_autonomy_run_ms,
            now_ms(),
            trigger,
            development_planning_needed,
        )
    {
        run_autonomy_exploration(workspace, tools, employee, trigger, false)?;
    }
    Ok(())
}

pub fn process_autonomy_tick(
    workspace: &Path,
    tools: &crate::tools::manager::ToolManager,
) -> anyhow::Result<()> {
    // Check shop status - skip autonomy when closed
    if let Ok(status) = crate::shop_status::load_shop_status(workspace) {
        if !status.is_open {
            tracing::info!("shop is closed, skipping autonomy tick");
            return Ok(());
        }
    }
    let employees = list_employee_records(workspace)?;
    let pending = list_pending_autonomy_employees(workspace)?;
    for employee in employees {
        let trigger = if pending.iter().any(|id| id == &employee.id) {
            AutonomyTriggerKind::TaskCompleted
        } else {
            AutonomyTriggerKind::Scheduled
        };
        let _ = process_employee_autonomy(workspace, tools, &employee, trigger);
    }
    Ok(())
}

#[derive(Clone)]
pub struct AutonomyCoordinator {
    workspace: Arc<RwLock<crate::WorkspaceState>>,
    tools: Arc<RwLock<crate::tools::manager::ToolManager>>,
    notify: Arc<Notify>,
    last_tick_ms: Arc<Mutex<Option<u64>>>,
}

impl AutonomyCoordinator {
    pub fn new(
        workspace: Arc<RwLock<crate::WorkspaceState>>,
        tools: Arc<RwLock<crate::tools::manager::ToolManager>>,
    ) -> Self {
        let notify = Arc::new(Notify::new());
        crate::autonomy_trigger::register_autonomy_notify(notify.clone());
        Self {
            workspace,
            tools,
            notify,
            last_tick_ms: Arc::new(Mutex::new(None)),
        }
    }

    pub fn status(&self) -> AutonomyStatusWire {
        let pending = self
            .workspace
            .read()
            .expect("workspace lock poisoned")
            .path
            .as_ref()
            .and_then(|ws| list_pending_autonomy_employees(ws).ok())
            .unwrap_or_default();
        AutonomyStatusWire {
            enabled: true,
            interval_secs: AUTONOMY_INTERVAL_SECS,
            pending_employees: pending,
            last_tick_ms: *self.last_tick_ms.lock().expect("autonomy tick lock"),
        }
    }

    pub async fn run_loop(self: Arc<Self>) {
        if let Err(err) = self.run_tick().await {
            tracing::warn!("autonomy initial tick failed: {err}");
        }
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(AUTONOMY_INTERVAL_SECS)) => {}
                _ = self.notify.notified() => {}
            }
            if let Err(err) = self.run_tick().await {
                tracing::warn!("autonomy tick failed: {err}");
            }
        }
    }

    async fn run_tick(&self) -> anyhow::Result<()> {
        let workspace = self
            .workspace
            .read()
            .expect("workspace lock poisoned")
            .path
            .clone();
        let Some(workspace) = workspace else {
            return Ok(());
        };
        let tools = self.tools.read().expect("tools lock poisoned").clone();
        tokio::task::spawn_blocking(move || process_autonomy_tick(&workspace, &tools))
            .await??;
        *self.last_tick_ms.lock().expect("autonomy tick lock") = Some(now_ms());
        Ok(())
    }
}

fn workspace_root(state: &AppState) -> Option<PathBuf> {
    state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone()
}

pub async fn get_autonomy_status(
    State(state): State<AppState>,
) -> Json<AutonomyStatusWire> {
    Json(
        state
            .autonomy
            .as_ref()
            .map(|c| c.status())
            .unwrap_or(AutonomyStatusWire {
                enabled: false,
                interval_secs: AUTONOMY_INTERVAL_SECS,
                pending_employees: vec![],
                last_tick_ms: None,
            }),
    )
}

pub async fn run_autonomy_tick_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<AutonomyStatusWire>, (axum::http::StatusCode, String)> {
    let Some(coordinator) = state.autonomy.clone() else {
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            crate::i18n::msg(&headers, "autonomy_not_enabled"),
        ));
    };
    coordinator
        .run_tick()
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(coordinator.status()))
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
    load_todos(&workspace, &employee_id)
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn run_employee_autonomy_handler(
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
    let employees = list_employee_records(&workspace).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    let Some(employee) = employees.into_iter().find(|e| e.id == employee_id) else {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            crate::i18n::msg(&headers, "employee_not_found"),
        ));
    };
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workspace_for_load = workspace.clone();
    tokio::task::spawn_blocking(move || {
        process_employee_autonomy(
            &workspace,
            &tools,
            &employee,
            AutonomyTriggerKind::Manual,
        )
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    load_todos(&workspace_for_load, &employee_id)
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub async fn run_employee_autonomy_explore_handler(
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
    let employees = list_employee_records(&workspace).map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    let Some(employee) = employees.into_iter().find(|e| e.id == employee_id) else {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            crate::i18n::msg(&headers, "employee_not_found"),
        ));
    };
    let tasks = TaskStore::new(&workspace).list().map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;
    if is_employee_busy(&tasks, &employee_id) {
        return Err((
            axum::http::StatusCode::CONFLICT,
            crate::i18n::msg(&headers, "employee_busy_cannot_explore"),
        ));
    }
    let tools = state.tools.read().expect("tools lock poisoned").clone();
    let workspace_for_load = workspace.clone();
    tokio::task::spawn_blocking(move || {
        run_autonomy_exploration(
            &workspace,
            &tools,
            &employee,
            AutonomyTriggerKind::Manual,
            true,
        )
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    load_todos(&workspace_for_load, &employee_id)
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        requirement::{RequirementPhase, RequirementSummary},
        tasks::{AgentTaskRecord, CodeAgentTaskParams, TaskKind, TaskStatus},
    };

    fn sample_summary(id: &str, phase: RequirementPhase) -> RequirementSummary {
        RequirementSummary {
            id: id.into(),
            title: format!("Title {id}"),
            phase,
            confirm_status: None,
            created_at_ms: 1,
            updated_at_ms: 2,
            dir_path: format!("requirements/{id}"),
        }
    }

    fn sample_task(executor: &str, status: TaskStatus) -> AgentTaskRecord {
        let mut task = AgentTaskRecord::new(
            &CodeAgentTaskParams {
                kind: TaskKind::AutonomyExecute,
                content: "x".into(),
                workdir: std::path::PathBuf::from("/tmp"),
                messages: vec![],
                executor_id: Some(executor.into()),
                parent_task_id: None,
                context: serde_json::json!({}),
            },
            "t1".into(),
            1,
        );
        task.status = status;
        task
    }

    #[test]
    fn selects_git_exploration_when_no_in_progress_requirements() {
        let items = vec![sample_summary("r1", RequirementPhase::Release)];
        assert_eq!(
            select_exploration_mode(&items),
            ExplorationMode::GitExploration
        );
    }

    #[test]
    fn selects_requirement_planning_when_in_progress_exists() {
        let items = vec![sample_summary("r1", RequirementPhase::Development)];
        assert_eq!(
            select_exploration_mode(&items),
            ExplorationMode::RequirementPlanning
        );
    }

    #[test]
    fn should_run_autonomy_when_todos_empty_and_idle() {
        assert!(should_run_autonomy(
            0,
            false,
            None,
            100_000,
            AutonomyTriggerKind::Scheduled,
            false,
        ));
    }

    #[test]
    fn should_not_run_scheduled_autonomy_when_todos_remain() {
        assert!(!should_run_autonomy(
            2,
            false,
            None,
            100_000,
            AutonomyTriggerKind::Scheduled,
            false,
        ));
    }

    #[test]
    fn scheduled_autonomy_runs_when_development_planning_needed() {
        assert!(should_run_autonomy(
            2,
            false,
            None,
            100_000,
            AutonomyTriggerKind::Scheduled,
            true,
        ));
    }

    #[test]
    fn should_run_autonomy_after_task_completed_even_with_debounce() {
        assert!(should_run_autonomy(
            3,
            false,
            Some(99_000),
            105_000,
            AutonomyTriggerKind::TaskCompleted,
            false,
        ));
    }

    #[test]
    fn is_employee_busy_when_running_task_exists() {
        let tasks = vec![
            sample_task("alice", TaskStatus::Completed),
            sample_task("alice", TaskStatus::Running),
        ];
        assert!(is_employee_busy(&tasks, "alice"));
        assert!(!is_employee_busy(&tasks, "bob"));
    }

    #[test]
    fn parse_autonomy_plan_from_json_line() {
        let output = r#"{"mode":"requirement_planning","todos":[{"title":"Draft test plan","description":"Cover auth flows"}],"new_requirements":[]}"#;
        let plan = parse_autonomy_plan_output(output).expect("plan");
        assert_eq!(plan.todos.len(), 1);
        assert_eq!(plan.todos[0].title, "Draft test plan");
    }

    #[test]
    fn parse_autonomy_plan_from_json_codeblock() {
        let output = r#"Here is the plan:
```json
{"mode":"requirement_planning","todos":[{"title":"Build API","description":"Add endpoints","requirement_id":"auth","requirement_phase":"development"}],"new_requirements":[]}
```"#;
        let plan = parse_autonomy_plan_output(output).expect("plan");
        assert_eq!(plan.todos.len(), 1);
        assert_eq!(plan.todos[0].requirement_id.as_deref(), Some("auth"));
    }

    #[test]
    fn apply_autonomy_plan_creates_development_tasks() {
        use crate::requirement::{format_requirement_md, RequirementMeta, REQUIREMENT_FILE};

        let workspace = std::env::temp_dir().join(format!(
            "kaisha-autonomy-dev-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let employee_dir = crate::employee::employee_root(&workspace).join("dev1");
        std::fs::create_dir_all(&employee_dir).unwrap();
        let req_dir = ensure_requirements_root(&workspace).unwrap().join("auth");
        std::fs::create_dir_all(&req_dir).unwrap();
        std::fs::write(
            req_dir.join(REQUIREMENT_FILE),
            format_requirement_md(
                &RequirementMeta {
                    id: "auth".into(),
                    title: "User auth".into(),
                    phase: RequirementPhase::Development,
                    confirm_status: None,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                },
                "## Scope\nImplement login.",
            ),
        )
        .unwrap();
        let plan = ParsedAutonomyPlan {
            _mode: Some("requirement_planning".into()),
            todos: vec![ParsedAutonomyTodo {
                title: "Implement login API".into(),
                description: "Add login endpoint".into(),
                requirement_id: Some("auth".into()),
                requirement_phase: Some("development".into()),
            }],
            new_requirements: vec![],
        };
        let added = apply_autonomy_plan(
            &workspace,
            "dev1",
            ExplorationMode::RequirementPlanning,
            &plan,
        )
        .unwrap();
        assert_eq!(added, 1);
        let dev_state = crate::requirement_development::try_load_dev_state(&workspace, "auth")
            .expect("dev state");
        assert_eq!(dev_state.tasks.len(), 1);
        assert_eq!(dev_state.tasks[0].title, "Implement login API");
        assert_eq!(dev_state.tasks[0].assignee.as_deref(), Some("dev1"));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn apply_autonomy_plan_adds_todos_and_requirements() {
        let workspace = std::env::temp_dir().join(format!(
            "kaisha-autonomy-apply-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let employee_dir = crate::employee::employee_root(&workspace).join("dev1");
        std::fs::create_dir_all(&employee_dir).unwrap();
        let plan = ParsedAutonomyPlan {
            _mode: Some("git_exploration".into()),
            todos: vec![ParsedAutonomyTodo {
                title: "Fix nav contrast".into(),
                description: "Improve accessibility".into(),
                requirement_id: None,
                requirement_phase: None,
            }],
            new_requirements: vec![ParsedNewRequirement {
                id: Some("ux-nav-contrast".into()),
                title: "Improve nav contrast".into(),
                phase: Some("collection".into()),
                content: Some("## Problem\nLow contrast.".into()),
            }],
        };
        let added = apply_autonomy_plan(
            &workspace,
            "dev1",
            ExplorationMode::GitExploration,
            &plan,
        )
        .unwrap();
        assert_eq!(added, 1);
        let file = load_todos(&workspace, "dev1").unwrap();
        assert_eq!(file.items.len(), 1);
        assert!(workspace
            .join("requirements/ux-nav-contrast/requirement.md")
            .exists());
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn forced_manual_exploration_bypasses_debounce() {
        let now = 10_000u64;
        let last = Some(9_000u64);
        assert!(!should_start_autonomy_exploration(
            0,
            false,
            last,
            now,
            AutonomyTriggerKind::Manual,
            false,
            false,
        ));
        assert!(should_start_autonomy_exploration(
            0,
            false,
            last,
            now,
            AutonomyTriggerKind::Manual,
            true,
            false,
        ));
    }
}
