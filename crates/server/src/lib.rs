pub mod logging;
mod employee;
mod employee_chat;
mod employee_conversation_stream;
mod employee_intent_router;
mod employee_requirement_agent;
mod employee_todo;
mod agent_locale;
mod autonomy;
mod autonomy_task;
mod autonomy_trigger;
mod conversation_task;
mod dev_task_executor;
mod git;
mod i18n;
mod intent;
mod requirement;
mod requirement_agents;
mod requirement_development;
mod requirement_release;
mod requirement_review;
mod requirement_testing;
mod shop_status;
mod tasks;
mod tools;
mod work_rules;
mod work_task;
pub mod work_task_reconcile;

use application::HealthService;
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use domain::{SetWorkspaceRequest, WorkspaceSource, WorkspaceStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, RwLock},
};
use tower_http::cors::{Any, CorsLayer};
use tools::{
    manager::ToolManager,
    model::{CreateToolInstanceRequest, PatchToolInstanceEnabledRequest, ToolCatalogItem, ToolInstance, UpdateToolInstanceRequest},
};

const SETTINGS_MENUS: [&str; 4] = ["tools", "departments", "roles", "employees"];

#[derive(Clone)]
struct AppState {
    health: HealthService,
    workspace: Arc<RwLock<WorkspaceState>>,
    settings: Arc<RwLock<SettingsState>>,
    tools: Arc<RwLock<ToolManager>>,
    autonomy: Option<Arc<autonomy::runtime::AutonomousRuntime>>,
    shop_status: Arc<RwLock<shop_status::ShopStatus>>,
}

async fn health(State(state): State<AppState>) -> Json<domain::HealthStatus> {
    Json(state.health.get_status())
}

#[derive(Debug, Clone, Serialize)]
struct ShopStatusResponse {
    pub is_open: bool,
}

async fn get_shop_status(State(state): State<AppState>) -> Json<ShopStatusResponse> {
    let status = state.shop_status.read().expect("shop_status lock poisoned");
    Json(ShopStatusResponse {
        is_open: status.is_open,
    })
}

async fn toggle_shop_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<ShopStatusResponse>, (axum::http::StatusCode, String)> {
    let Some(workspace) = state.workspace.read().expect("workspace lock poisoned").path.clone() else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let status = shop_status::toggle_shop_status(&workspace)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut state_status = state.shop_status.write().expect("shop_status lock poisoned");
    *state_status = status.clone();
    Ok(Json(ShopStatusResponse {
        is_open: status.is_open,
    }))
}

#[derive(Clone)]
pub struct WorkspaceInit {
    pub path: Option<PathBuf>,
    pub source: WorkspaceSource,
    pub config_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceConfigFile {
    path: String,
}

#[derive(Clone)]
pub(crate) struct WorkspaceState {
    source: WorkspaceSource,
    path: Option<PathBuf>,
    config_file: PathBuf,
}

impl WorkspaceState {
    fn to_status(&self) -> WorkspaceStatus {
        WorkspaceStatus {
            configured: self.path.is_some(),
            path: self.path.as_ref().map(|p| p.to_string_lossy().to_string()),
            source: self.source.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SettingsMenuResponse {
    menu: String,
    count: usize,
    items: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
struct SettingsItemResponse {
    menu: String,
    address: String,
    value: Value,
}

#[derive(Debug, Clone, Deserialize)]
struct UpsertSettingsItemRequest {
    value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MenuConfigFile {
    version: u32,
    items: BTreeMap<String, Value>,
}

impl Default for MenuConfigFile {
    fn default() -> Self {
        Self {
            version: 1,
            items: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Default)]
struct SettingsState {
    workspace: Option<PathBuf>,
    menus: BTreeMap<String, MenuMemory>,
}

#[derive(Clone, Default)]
struct MenuMemory {
    file_path: Option<PathBuf>,
    items: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
enum SettingsMenu {
    Tools,
    Departments,
    Roles,
    Employees,
    WorkRules,
}

impl SettingsMenu {
    fn as_str(&self) -> &'static str {
        match self {
            SettingsMenu::Tools => "tools",
            SettingsMenu::Departments => "departments",
            SettingsMenu::Roles => "roles",
            SettingsMenu::Employees => "employees",
            SettingsMenu::WorkRules => "work_rules",
        }
    }
}

impl FromStr for SettingsMenu {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tools" => Ok(Self::Tools),
            "departments" => Ok(Self::Departments),
            "roles" => Ok(Self::Roles),
            "employees" => Ok(Self::Employees),
            "work_rules" => Ok(Self::WorkRules),
            _ => Err("unsupported settings menu"),
        }
    }
}

pub fn resolve_workspace_from_env(env_name: &str, config_file: PathBuf) -> anyhow::Result<WorkspaceInit> {
    if let Ok(value) = std::env::var(env_name) {
        let path = normalize_workspace_path(PathBuf::from(value.trim()))?;
        fs::create_dir_all(&path)?;
        return Ok(WorkspaceInit {
            path: Some(path),
            source: WorkspaceSource::Env,
            config_file,
        });
    }

    if config_file.exists() {
        let raw = fs::read_to_string(&config_file)?;
        let config: WorkspaceConfigFile = serde_json::from_str(&raw)?;
        let path = normalize_workspace_path(PathBuf::from(config.path))?;
        fs::create_dir_all(&path)?;
        return Ok(WorkspaceInit {
            path: Some(path),
            source: WorkspaceSource::Config,
            config_file,
        });
    }

    let path = default_workspace_path()?;
    fs::create_dir_all(&path)?;
    Ok(WorkspaceInit {
        path: Some(path),
        source: WorkspaceSource::Config,
        config_file,
    })
}

fn normalize_workspace_path(path: PathBuf) -> anyhow::Result<PathBuf> {
    let trimmed = PathBuf::from(path.to_string_lossy().trim().to_string());
    if trimmed.as_os_str().is_empty() {
        anyhow::bail!("workspace path cannot be empty");
    }

    Ok(if trimmed.is_absolute() {
        trimmed
    } else {
        std::env::current_dir()?.join(trimmed)
    })
}

fn default_workspace_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME is not set"))?;
    normalize_workspace_path(PathBuf::from(home).join(".kaisha"))
}

async fn get_workspace(State(state): State<AppState>) -> Json<WorkspaceStatus> {
    let workspace = state.workspace.read().expect("workspace lock poisoned");
    Json(workspace.to_status())
}

async fn set_workspace(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<SetWorkspaceRequest>,
) -> Result<Json<WorkspaceStatus>, (axum::http::StatusCode, String)> {
    let mut workspace = state.workspace.write().expect("workspace lock poisoned");

    if matches!(workspace.source, WorkspaceSource::Env) {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_env_controlled"),
        ));
    }

    let normalized = normalize_workspace_path(PathBuf::from(payload.path.trim()))
        .map_err(|err| (axum::http::StatusCode::BAD_REQUEST, err.to_string()))?;
    fs::create_dir_all(&normalized)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    persist_workspace_config(&workspace.config_file, &normalized)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let mut settings = state.settings.write().expect("settings lock poisoned");
    *settings = load_settings_state(Some(normalized.clone()))
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let mut tools = state.tools.write().expect("tools lock poisoned");
    tools
        .reload(Some(&normalized))
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    employee::ensure_default_employee(&normalized)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    git::ensure_workspace_repos(&normalized)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    work_rules::ensure_work_rules(&normalized)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    workspace.path = Some(normalized);
    workspace.source = WorkspaceSource::Config;
    Ok(Json(workspace.to_status()))
}

async fn get_tool_catalog(State(state): State<AppState>) -> Json<Vec<ToolCatalogItem>> {
    let tools = state.tools.read().expect("tools lock poisoned");
    Json(tools.catalog())
}

async fn list_tool_instances(State(state): State<AppState>) -> Json<Vec<ToolInstance>> {
    let tools = state.tools.read().expect("tools lock poisoned");
    Json(tools.list())
}

async fn get_tool_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ToolInstance>, (axum::http::StatusCode, String)> {
    let tools = state.tools.read().expect("tools lock poisoned");
    let Some(instance) = tools.get(&id) else {
        return Err((axum::http::StatusCode::NOT_FOUND, "tool not found".to_string()));
    };
    Ok(Json(instance))
}

async fn create_tool_instance(
    State(state): State<AppState>,
    Json(req): Json<CreateToolInstanceRequest>,
) -> Result<Json<ToolInstance>, (axum::http::StatusCode, String)> {
    let mut tools = state.tools.write().expect("tools lock poisoned");
    tools
        .create(req)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::BAD_REQUEST, err.to_string()))
}

async fn update_tool_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<UpdateToolInstanceRequest>,
) -> Result<Json<ToolInstance>, (axum::http::StatusCode, String)> {
    let mut tools = state.tools.write().expect("tools lock poisoned");
    tools
        .update(&id, req)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::BAD_REQUEST, err.to_string()))
}

async fn patch_tool_instance_enabled(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<PatchToolInstanceEnabledRequest>,
) -> Result<Json<ToolInstance>, (axum::http::StatusCode, String)> {
    let mut tools = state.tools.write().expect("tools lock poisoned");
    tools
        .set_enabled(&id, req.enabled)
        .map(Json)
        .map_err(|err| (axum::http::StatusCode::BAD_REQUEST, err.to_string()))
}

fn persist_workspace_config(config_file: &Path, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = config_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let payload = WorkspaceConfigFile {
        path: path.to_string_lossy().to_string(),
    };
    fs::write(config_file, serde_json::to_string_pretty(&payload)?)?;
    Ok(())
}

fn load_settings_state(workspace: Option<PathBuf>) -> anyhow::Result<SettingsState> {
    let mut state = SettingsState {
        workspace,
        menus: BTreeMap::new(),
    };

    if let Some(workdir) = state.workspace.as_ref() {
        let settings_root = workdir.join("settings");
        fs::create_dir_all(&settings_root)?;

        for menu in SETTINGS_MENUS {
            let menu_dir = settings_root.join(menu);
            let menu_file = menu_dir.join("config.yml");
            fs::create_dir_all(&menu_dir)?;
            if !menu_file.exists() {
                let initial = serde_yaml::to_string(&MenuConfigFile::default())?;
                fs::write(&menu_file, initial)?;
            }

            let loaded: MenuConfigFile = serde_yaml::from_str(&fs::read_to_string(&menu_file)?)?;
            state.menus.insert(
                menu.to_string(),
                MenuMemory {
                    file_path: Some(menu_file),
                    items: loaded.items,
                },
            );
        }
    } else {
        for menu in SETTINGS_MENUS {
            state.menus.insert(menu.to_string(), MenuMemory::default());
        }
    }

    Ok(state)
}

fn validate_address(address: &str) -> anyhow::Result<()> {
    if address.trim().is_empty() {
        anyhow::bail!("address_empty");
    }
    if !address.contains('.') {
        anyhow::bail!("address_format");
    }

    for part in address.split('.') {
        if part.is_empty() {
            anyhow::bail!("address_empty_segment");
        }
        if !part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            anyhow::bail!("address_segment_invalid");
        }
    }
    Ok(())
}

fn persist_menu(memory: &MenuMemory) -> anyhow::Result<()> {
    let Some(path) = memory.file_path.as_ref() else {
        anyhow::bail!("workspace is not configured");
    };
    let content = MenuConfigFile {
        version: 1,
        items: memory.items.clone(),
    };
    fs::write(path, serde_yaml::to_string(&content)?)?;
    Ok(())
}

async fn get_settings_menu(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(menu): AxumPath<String>,
) -> Result<Json<SettingsMenuResponse>, (axum::http::StatusCode, String)> {
    let parsed = SettingsMenu::from_str(&menu)
        .map_err(|_| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "unsupported_menu"),
            )
        })?;
    let settings = state.settings.read().expect("settings lock poisoned");
    let Some(memory) = settings.menus.get(parsed.as_str()) else {
        return Err((axum::http::StatusCode::NOT_FOUND, i18n::msg(&headers, "menu_not_found")));
    };

    Ok(Json(SettingsMenuResponse {
        menu: parsed.as_str().to_string(),
        count: memory.items.len(),
        items: memory.items.clone(),
    }))
}

async fn get_settings_item(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath((menu, address)): AxumPath<(String, String)>,
) -> Result<Json<SettingsItemResponse>, (axum::http::StatusCode, String)> {
    let parsed = SettingsMenu::from_str(&menu)
        .map_err(|_| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "unsupported_menu"),
            )
        })?;
    validate_address(&address)
        .map_err(|err| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, &err.to_string()),
            )
        })?;

    let settings = state.settings.read().expect("settings lock poisoned");
    let Some(memory) = settings.menus.get(parsed.as_str()) else {
        return Err((axum::http::StatusCode::NOT_FOUND, i18n::msg(&headers, "menu_not_found")));
    };
    let Some(value) = memory.items.get(&address) else {
        return Err((axum::http::StatusCode::NOT_FOUND, i18n::msg(&headers, "address_not_found")));
    };
    Ok(Json(SettingsItemResponse {
        menu,
        address,
        value: value.clone(),
    }))
}

async fn upsert_settings_item(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath((menu, address)): AxumPath<(String, String)>,
    Json(payload): Json<UpsertSettingsItemRequest>,
) -> Result<Json<SettingsItemResponse>, (axum::http::StatusCode, String)> {
    let parsed = SettingsMenu::from_str(&menu)
        .map_err(|_| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, "unsupported_menu"),
            )
        })?;
    validate_address(&address)
        .map_err(|err| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                i18n::msg(&headers, &err.to_string()),
            )
        })?;

    let mut settings = state.settings.write().expect("settings lock poisoned");
    let Some(memory) = settings.menus.get_mut(parsed.as_str()) else {
        return Err((axum::http::StatusCode::NOT_FOUND, i18n::msg(&headers, "menu_not_found")));
    };

    if memory.file_path.is_none() {
        return Err((
            axum::http::StatusCode::CONFLICT,
            i18n::msg(&headers, "workspace_not_configured"),
        ));
    }

    memory.items.insert(address.clone(), payload.value.clone());
    persist_menu(memory)
        .map_err(|err| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(Json(SettingsItemResponse {
        menu,
        address,
        value: payload.value,
    }))
}

async fn sync_workspace_locale_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    if headers.get("x-lang").is_some() {
        if let Some(workspace) = state
            .workspace
            .read()
            .expect("workspace lock poisoned")
            .path
            .clone()
        {
            agent_locale::sync_lang_from_headers(&headers, &workspace);
        }
    }
    next.run(request).await
}

pub async fn run_http(addr: SocketAddr, workspace_init: WorkspaceInit) -> anyhow::Result<()> {
    if let Some(workspace) = workspace_init.path.as_deref() {
        employee::ensure_default_employee(workspace)?;
        git::ensure_workspace_repos(workspace)?;
        work_rules::ensure_work_rules(workspace)?;
    }
    let settings_state = load_settings_state(workspace_init.path.clone())?;
    let tools_manager = ToolManager::new(workspace_init.path.as_deref())?;
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let workspace = Arc::new(RwLock::new(WorkspaceState {
        source: workspace_init.source,
        path: workspace_init.path.clone(),
        config_file: workspace_init.config_file,
    }));
    let settings = Arc::new(RwLock::new(settings_state));
    let tools = Arc::new(RwLock::new(tools_manager));
    let shop_status_value = if let Some(wp) = &workspace_init.path {
        shop_status::load_shop_status(wp).unwrap_or_default()
    } else {
        shop_status::ShopStatus::default()
    };
    let shop_status = Arc::new(RwLock::new(shop_status_value));
    let coordinator = Arc::new(autonomy::runtime::AutonomousRuntime::new(
        workspace_init.path.clone().unwrap_or_default(),
        tools.clone(),
    ));
    let coordinator_clone = coordinator.clone();
    tokio::spawn(async move {
        if let Err(err) = coordinator_clone.initialize().await {
            tracing::warn!("autonomy runtime init failed: {err}");
        }
        coordinator_clone.run_loop().await;
    });
    let app_state = AppState {
        health: HealthService,
        workspace,
        settings,
        tools,
        autonomy: Some(coordinator),
        shop_status,
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/workspace", get(get_workspace).post(set_workspace))
        .route("/api/settings/:menu", get(get_settings_menu))
        .route(
            "/api/settings/:menu/:address",
            get(get_settings_item).put(upsert_settings_item),
        )
        .route(
            "/api/employees",
            get(employee::list_employees).post(employee::create_employee),
        )
        .route(
            "/api/employees/hire",
            post(employee::hire_employee),
        )
        .route(
            "/api/employees/archived",
            get(employee::list_archived_employees),
        )
        .route(
            "/api/employees/:id/fire",
            post(employee::fire_employee),
        )
        .route(
            "/api/employees/:id/reinstate",
            post(employee::reinstate_employee),
        )
        .route(
            "/api/employees/:id/handover",
            post(employee::handover_employee),
        )
        .route(
            "/api/employees/:id/hard-delete",
            post(employee::hard_delete_employee),
        )
        .route(
            "/api/employees/:id/messages",
            get(employee_chat::get_messages).post(employee_chat::post_message),
        )
        .route(
            "/api/employees/:id/messages/stream",
            post(employee_chat::post_message_stream),
        )
        .route(
            "/api/employees/:id/conversation/stream",
            get(employee_conversation_stream::conversation_stream_handler),
        )
        .route("/api/tools/catalog", get(get_tool_catalog))
        .route("/api/tools/instances", get(list_tool_instances).post(create_tool_instance))
        .route("/api/tools/instances/:id", get(get_tool_instance).put(update_tool_instance).patch(patch_tool_instance_enabled))
        .route("/api/git/repos", get(git::list_git_repos).post(git::create_git_repo))
        .route("/api/git/repos/:id", get(git::get_git_repo))
        .route("/api/git/repos/:id/op", axum::routing::post(git::run_git_operation))
        .route("/api/git/repos/:id/branches", get(git::list_git_branches))
        .route("/api/git/repos/:id/tree", get(git::list_git_tree))
        .route("/api/git/repos/:id/file", get(git::read_git_file))
        .route("/api/git/init", axum::routing::post(git::init_git_project))
        .route("/api/git/exec", axum::routing::post(git::exec_git))
        .route(
            "/api/requirements",
            get(requirement::list_requirements).post(requirement::create_requirement),
        )
        .route(
            "/api/requirements/:id",
            get(requirement::get_requirement).put(requirement::update_requirement),
        )
        .route(
            "/api/requirements/:id/optimize",
            post(requirement::optimize_requirement),
        )
        .route(
            "/api/requirements/:id/review",
            get(requirement_review::get_requirement_review)
                .post(requirement_review::start_requirement_review),
        )
        .route(
            "/api/requirements/:id/review/run",
            axum::routing::post(requirement_review::run_review_handler),
        )
        .route(
            "/api/requirements/:id/review/force-pass",
            axum::routing::post(requirement_review::force_pass_review_handler),
        )
        .route(
            "/api/requirements/:id/review/opinions/:employee_id/:action",
            axum::routing::post(requirement_review::opinion_action_handler),
        )
        .route(
            "/api/requirements/archived",
            get(requirement::list_archived_requirements),
        )
        .route(
            "/api/requirements/:id/abandon",
            post(requirement::abandon_requirement),
        )
        .route(
            "/api/requirements/:id/reinstate",
            post(requirement::reinstate_requirement),
        )
        .route(
            "/api/requirements/:id/hard-delete",
            post(requirement::hard_delete_requirement),
        )
        .route(
            "/api/requirements/:id/development",
            axum::routing::get(requirement_development::get_development)
                .post(requirement_development::start_development),
        )
        .route(
            "/api/requirements/:id/development/split",
            axum::routing::post(requirement_development::split_development_tasks),
        )
        .route(
            "/api/requirements/:id/development/tasks",
            axum::routing::post(requirement_development::create_task),
        )
        .route(
            "/api/requirements/:id/development/tasks/:task_id",
            axum::routing::put(requirement_development::update_task)
                .delete(requirement_development::delete_task),
        )
        .route(
            "/api/requirements/:id/development/tasks/:task_id/:action",
            axum::routing::post(requirement_development::task_action),
        )
        .route(
            "/api/requirements/:id/testing",
            axum::routing::get(requirement_testing::get_testing),
        )
        .route(
            "/api/requirements/:id/testing/split",
            axum::routing::post(requirement_testing::split_test_tasks),
        )
        .route(
            "/api/requirements/:id/testing/tasks/:task_id/:action",
            axum::routing::post(requirement_testing::test_task_action),
        )
        .route(
            "/api/requirements/:id/release",
            axum::routing::get(requirement_release::get_release),
        )
        .route(
            "/api/requirements/:id/release/package",
            axum::routing::post(requirement_release::package_release),
        )
        .route(
            "/api/requirements/:id/release/start",
            axum::routing::post(requirement_release::start_release),
        )
        .route(
            "/api/work-rules",
            get(work_rules::get_work_rules).put(work_rules::put_work_rules),
        )
        .route("/api/work-tasks", get(work_task::list_work_tasks_handler))
        .route(
            "/api/work-tasks/:id",
            get(work_task::get_work_task_handler),
        )
        .route("/api/tasks", get(tasks::list_tasks))
        .route("/api/tasks/stop-all", post(tasks::stop_all_tasks))
        .route("/api/tasks/:id/detail", get(tasks::get_task_detail))
        .route("/api/tasks/:id", get(tasks::get_task))
        .route("/api/tasks/:id/rerun", post(tasks::rerun_task))
        .route("/api/tasks/:id/stop", post(tasks::stop_task))
        .route("/api/tasks/:id/alive", get(tasks::get_task_alive_status))
        .route("/api/autonomy/status", get(autonomy::api::get_autonomy_status))
        .route("/api/autonomy/tick", post(autonomy::api::run_autonomy_tick_handler))
        .route(
            "/api/employees/:id/todos",
            get(autonomy::api::list_employee_todos_handler),
        )
        .route(
            "/api/employees/:id/autonomy/run",
            post(autonomy::api::run_employee_autonomy_handler),
        )
        .route(
            "/api/employees/:id/autonomy/explore",
            post(autonomy::api::run_employee_autonomy_explore_handler),
        )
        .route(
            "/api/employees/:id/autonomy/explore/stream",
            post(autonomy::api::run_employee_autonomy_explore_stream_handler),
        )
        .route(
            "/api/employees/:id/autonomy/run/stream",
            post(autonomy::api::run_employee_autonomy_run_stream_handler),
        )
        .route("/api/autonomy/tasks", get(autonomy::api::list_tasks_handler))
        .route("/api/autonomy/plans", get(autonomy::api::list_plans_handler))
        .route("/api/autonomy/workers", get(autonomy::api::list_workers_handler))
        .route("/api/shop/status", get(get_shop_status))
        .route("/api/shop/toggle", post(toggle_shop_status))
        .layer(cors)
        .layer(logging::http_trace_layer())
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            sync_workspace_locale_middleware,
        ))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("HTTP API listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_workspace_from_env;
    use domain::WorkspaceSource;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn uses_home_kaisha_when_workspace_is_unset() {
        let home = std::env::var("HOME").expect("HOME must be set for test");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        let config_file = std::env::temp_dir().join(format!("kaisha-workspace-{unique}.json"));
        if config_file.exists() {
            fs::remove_file(&config_file).expect("failed to cleanup stale temp config file");
        }

        let init = resolve_workspace_from_env("KAISHA_WORKDIR_TEST_FALLBACK", config_file)
            .expect("workspace fallback should resolve");

        assert_eq!(init.path, Some(PathBuf::from(home).join(".kaisha")));
        assert!(matches!(init.source, WorkspaceSource::Config));
    }
}
