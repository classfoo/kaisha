use crate::tools::model::{ToolIndexFile, ToolInstance};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct ToolStore {
    index_path: Option<PathBuf>,
}

impl ToolStore {
    pub fn new(workspace: Option<&Path>) -> anyhow::Result<Self> {
        if let Some(workspace) = workspace {
            let dir = workspace.join("settings").join("tools");
            fs::create_dir_all(&dir)?;
            let index_path = dir.join("index.yml");
            if !index_path.exists() {
                fs::write(&index_path, serde_yaml::to_string(&ToolIndexFile::default())?)?;
            }
            return Ok(Self {
                index_path: Some(index_path),
            });
        }
        Ok(Self { index_path: None })
    }

    pub fn load(&self) -> anyhow::Result<BTreeMap<String, ToolInstance>> {
        let Some(path) = self.index_path.as_ref() else {
            return Ok(BTreeMap::new());
        };
        let raw = fs::read_to_string(path)?;
        let parsed: ToolIndexFile = serde_yaml::from_str(&raw)?;
        Ok(parsed.instances)
    }

    pub fn save(&self, instances: &BTreeMap<String, ToolInstance>) -> anyhow::Result<()> {
        let Some(path) = self.index_path.as_ref() else {
            anyhow::bail!("workspace is not configured");
        };
        let payload = ToolIndexFile {
            instances: instances.clone(),
        };
        fs::write(path, serde_yaml::to_string(&payload)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::model::{ToolInstance, ToolKind};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-tool-store-{unique}"))
    }

    #[test]
    fn load_returns_empty_without_workspace() {
        let store = ToolStore::new(None).unwrap();
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();
        let store = ToolStore::new(Some(&workspace)).unwrap();
        let mut instances = BTreeMap::new();
        instances.insert(
            "tool_1".into(),
            ToolInstance {
                id: "tool_1".into(),
                kind: ToolKind::ClaudeCode,
                name: "Claude".into(),
                enabled: true,
                version: 1,
                config: serde_json::json!({}),
            },
        );
        store.save(&instances).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get("tool_1").unwrap().name, "Claude");
        let _ = fs::remove_dir_all(&workspace);
    }
}
