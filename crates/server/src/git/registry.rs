use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const REPOS_DIR: &str = "repos";
pub const MAIN_REPO_ID: &str = "main";
const REGISTRY_FILE: &str = "registry.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepoRecord {
    pub id: String,
    pub name: String,
    pub is_main: bool,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RegistryFile {
    version: u32,
    main_repo_id: String,
    repos: Vec<GitRepoRecord>,
}

impl Default for RegistryFile {
    fn default() -> Self {
        Self {
            version: 1,
            main_repo_id: MAIN_REPO_ID.to_string(),
            repos: vec![],
        }
    }
}

pub fn repos_root(workspace: &Path) -> PathBuf {
    workspace.join(REPOS_DIR)
}

fn registry_path(workspace: &Path) -> PathBuf {
    repos_root(workspace).join(REGISTRY_FILE)
}

pub fn repo_dir(workspace: &Path, repo_id: &str) -> PathBuf {
    repos_root(workspace).join(repo_id)
}

pub fn validate_repo_id(raw: &str) -> anyhow::Result<String> {
    let id = raw.trim();
    if id.is_empty() {
        anyhow::bail!("git_repo_id_empty");
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        anyhow::bail!("git_repo_id_invalid");
    }
    Ok(id.to_string())
}

pub fn validate_repo_name(raw: &str) -> anyhow::Result<String> {
    let name = raw.trim();
    if name.is_empty() {
        anyhow::bail!("git_repo_name_empty");
    }
    Ok(name.to_string())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn load_registry(workspace: &Path) -> anyhow::Result<RegistryFile> {
    let path = registry_path(workspace);
    if !path.exists() {
        return Ok(RegistryFile::default());
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn save_registry(workspace: &Path, registry: &RegistryFile) -> anyhow::Result<()> {
    let root = repos_root(workspace);
    fs::create_dir_all(&root)?;
    fs::write(registry_path(workspace), serde_json::to_string_pretty(registry)?)?;
    Ok(())
}

pub fn list_repos(workspace: &Path) -> anyhow::Result<Vec<GitRepoRecord>> {
    let registry = load_registry(workspace)?;
    Ok(registry.repos)
}

pub fn find_repo(workspace: &Path, repo_id: &str) -> anyhow::Result<GitRepoRecord> {
    let id = validate_repo_id(repo_id)?;
    let registry = load_registry(workspace)?;
    registry
        .repos
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| anyhow::anyhow!("git_repo_not_found"))
}

pub fn add_repo(workspace: &Path, id: &str, name: &str, is_main: bool) -> anyhow::Result<GitRepoRecord> {
    let id = validate_repo_id(id)?;
    let name = validate_repo_name(name)?;
    let mut registry = load_registry(workspace)?;
    if registry.repos.iter().any(|r| r.id == id) {
        anyhow::bail!("git_repo_already_exists");
    }
    let record = GitRepoRecord {
        id: id.clone(),
        name,
        is_main,
        created_at_ms: now_ms(),
    };
    if is_main {
        for r in &mut registry.repos {
            r.is_main = false;
        }
        registry.main_repo_id = id.clone();
    }
    registry.repos.push(record.clone());
    save_registry(workspace, &registry)?;
    Ok(record)
}

pub fn ensure_main_repo(workspace: &Path) -> anyhow::Result<GitRepoRecord> {
    fs::create_dir_all(repos_root(workspace))?;
    let mut registry = load_registry(workspace)?;
    if let Some(existing) = registry.repos.iter().find(|r| r.id == MAIN_REPO_ID).cloned() {
        let dir = repo_dir(workspace, MAIN_REPO_ID);
        if !dir.join(".git").exists() {
            fs::create_dir_all(&dir)?;
            super::service::git_init(&dir)?;
        }
        return Ok(existing);
    }

    let record = GitRepoRecord {
        id: MAIN_REPO_ID.to_string(),
        name: "Main".to_string(),
        is_main: true,
        created_at_ms: now_ms(),
    };
    registry.main_repo_id = MAIN_REPO_ID.to_string();
    for r in &mut registry.repos {
        r.is_main = false;
    }
    registry.repos.push(record.clone());
    save_registry(workspace, &registry)?;

    let dir = repo_dir(workspace, MAIN_REPO_ID);
    fs::create_dir_all(&dir)?;
    if !dir.join(".git").exists() {
        super::service::git_init(&dir)?;
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }

    #[test]
    fn ensure_main_repo_creates_registry_and_git_dir() {
        let workspace = unique_temp_dir("kaisha-git-main");
        let record = ensure_main_repo(&workspace).expect("ensure main");
        assert_eq!(record.id, MAIN_REPO_ID);
        assert!(record.is_main);
        assert!(repo_dir(&workspace, MAIN_REPO_ID).join(".git").exists());
    }
}
