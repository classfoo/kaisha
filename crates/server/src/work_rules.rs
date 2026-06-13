use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const WORK_RULES_SETTINGS_MENU: &str = "work_rules";
const CONFIG_FILE: &str = "config.yml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRulesFile {
    pub version: u32,
    pub roles: BTreeMap<String, WorkRoleDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRoleDefinition {
    pub display_name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub duties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRulesWire {
    pub version: u32,
    pub roles: BTreeMap<String, WorkRoleDefinition>,
}

fn settings_root(workspace: &Path) -> PathBuf {
    workspace.join("settings").join(WORK_RULES_SETTINGS_MENU)
}

fn config_path(workspace: &Path) -> PathBuf {
    settings_root(workspace).join(CONFIG_FILE)
}

pub fn default_work_rules() -> WorkRulesFile {
    let mut roles = BTreeMap::new();
    roles.insert(
        "product".to_string(),
        WorkRoleDefinition {
            display_name: "Product".to_string(),
            aliases: vec![
                "product".into(),
                "产品".into(),
                "产品经理".into(),
                "pm".into(),
            ],
            duties: phase_duties(
                "Clarify user value and scope",
                "Break down user stories",
                "Validate shipped behavior vs goals",
                "Publish release notes and rollout plan",
            ),
        },
    );
    roles.insert(
        "engineering".to_string(),
        WorkRoleDefinition {
            display_name: "Engineering".to_string(),
            aliases: vec![
                "engineering".into(),
                "研发".into(),
                "开发".into(),
                "engineer".into(),
                "rd".into(),
            ],
            duties: phase_duties(
                "Estimate feasibility and dependencies",
                "Implement and document changes",
                "Support test automation and defect fixes",
                "Operate and monitor production",
            ),
        },
    );
    roles.insert(
        "testing".to_string(),
        WorkRoleDefinition {
            display_name: "Testing".to_string(),
            aliases: vec![
                "testing".into(),
                "测试".into(),
                "qa".into(),
                "quality".into(),
            ],
            duties: phase_duties(
                "Identify test scenarios early",
                "Execute tests and report defects",
                "Regression and release verification",
                "Post-release quality summary",
            ),
        },
    );
    roles.insert(
        "operations".to_string(),
        WorkRoleDefinition {
            display_name: "Operations".to_string(),
            aliases: vec![
                "operations".into(),
                "运营".into(),
                "ops".into(),
            ],
            duties: phase_duties(
                "Capture operational constraints",
                "Coordinate launch activities",
                "Smoke test in production-like env",
                "Runbook and incident readiness",
            ),
        },
    );
    WorkRulesFile { version: 1, roles }
}

fn phase_duties(
    collection: &str,
    development: &str,
    testing: &str,
    release: &str,
) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("collection".into(), collection.into());
    m.insert("development".into(), development.into());
    m.insert("testing".into(), testing.into());
    m.insert("release".into(), release.into());
    m
}

pub fn ensure_work_rules(workspace: &Path) -> anyhow::Result<()> {
    let dir = settings_root(workspace);
    fs::create_dir_all(&dir)?;
    let path = config_path(workspace);
    if !path.exists() {
        save_work_rules(workspace, &default_work_rules())?;
    }
    Ok(())
}

pub fn load_work_rules(workspace: &Path) -> anyhow::Result<WorkRulesFile> {
    ensure_work_rules(workspace)?;
    let raw = fs::read_to_string(config_path(workspace))?;
    Ok(serde_yaml::from_str(&raw)?)
}

pub fn save_work_rules(workspace: &Path, rules: &WorkRulesFile) -> anyhow::Result<()> {
    let dir = settings_root(workspace);
    fs::create_dir_all(&dir)?;
    fs::write(config_path(workspace), serde_yaml::to_string(rules)?)?;
    Ok(())
}

pub fn resolve_role_key(rules: &WorkRulesFile, employee_role: &str) -> Option<String> {
    let needle = employee_role.trim().to_lowercase();
    if needle.is_empty() {
        return None;
    }
    for (key, role) in &rules.roles {
        if key.to_lowercase() == needle {
            return Some(key.clone());
        }
        for alias in &role.aliases {
            if alias.trim().to_lowercase() == needle {
                return Some(key.clone());
            }
        }
        if role.display_name.trim().to_lowercase() == needle {
            return Some(key.clone());
        }
    }
    None
}

pub fn duty_for_phase(rules: &WorkRulesFile, role_key: &str, phase: &str) -> String {
    rules
        .roles
        .get(role_key)
        .and_then(|r| r.duties.get(phase))
        .cloned()
        .unwrap_or_else(|| "Participate according to your role.".to_string())
}

pub async fn get_work_rules(
    headers: axum::http::HeaderMap,
    axum::extract::State(state): axum::extract::State<crate::AppState>,
) -> Result<axum::Json<WorkRulesWire>, (axum::http::StatusCode, String)> {
    let workspace = state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone();
    let Some(workspace) = workspace else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            crate::i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let rules = load_work_rules(&workspace)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(axum::Json(WorkRulesWire {
        version: rules.version,
        roles: rules.roles,
    }))
}

pub async fn put_work_rules(
    headers: axum::http::HeaderMap,
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::Json(payload): axum::Json<WorkRulesWire>,
) -> Result<axum::Json<WorkRulesWire>, (axum::http::StatusCode, String)> {
    let workspace = state
        .workspace
        .read()
        .expect("workspace lock poisoned")
        .path
        .clone();
    let Some(workspace) = workspace else {
        return Err((
            axum::http::StatusCode::CONFLICT,
            crate::i18n::msg(&headers, "workspace_not_configured"),
        ));
    };
    let file = WorkRulesFile {
        version: payload.version,
        roles: payload.roles.clone(),
    };
    save_work_rules(&workspace, &file)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(axum::Json(payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rules_include_four_roles() {
        let rules = default_work_rules();
        assert!(rules.roles.contains_key("product"));
        assert!(rules.roles.contains_key("engineering"));
        assert!(rules.roles.contains_key("testing"));
        assert!(rules.roles.contains_key("operations"));
    }

    #[test]
    fn resolves_chinese_role_alias() {
        let rules = default_work_rules();
        assert_eq!(resolve_role_key(&rules, "研发"), Some("engineering".to_string()));
    }
}
