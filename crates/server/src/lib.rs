mod i18n;

use application::HealthService;
use axum::{
    extract::{Path as AxumPath, State},
    http::HeaderMap,
    routing::get,
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

const SETTINGS_MENUS: [&str; 4] = ["tools", "departments", "roles", "employees"];

#[derive(Clone)]
struct AppState {
    health: HealthService,
    workspace: Arc<RwLock<WorkspaceState>>,
    settings: Arc<RwLock<SettingsState>>,
}

async fn health(State(state): State<AppState>) -> Json<domain::HealthStatus> {
    Json(state.health.get_status())
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
struct WorkspaceState {
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
}

impl SettingsMenu {
    fn as_str(&self) -> &'static str {
        match self {
            SettingsMenu::Tools => "tools",
            SettingsMenu::Departments => "departments",
            SettingsMenu::Roles => "roles",
            SettingsMenu::Employees => "employees",
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

    Ok(WorkspaceInit {
        path: None,
        source: WorkspaceSource::Unset,
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

    workspace.path = Some(normalized);
    workspace.source = WorkspaceSource::Config;
    Ok(Json(workspace.to_status()))
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

pub async fn run_http(addr: SocketAddr, workspace_init: WorkspaceInit) -> anyhow::Result<()> {
    let settings_state = load_settings_state(workspace_init.path.clone())?;
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/workspace", get(get_workspace).post(set_workspace))
        .route("/api/settings/:menu", get(get_settings_menu))
        .route(
            "/api/settings/:menu/:address",
            get(get_settings_item).put(upsert_settings_item),
        )
        .with_state(AppState {
            health: HealthService,
            workspace: Arc::new(RwLock::new(WorkspaceState {
                source: workspace_init.source,
                path: workspace_init.path,
                config_file: workspace_init.config_file,
            })),
            settings: Arc::new(RwLock::new(settings_state)),
        });

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("HTTP API listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
