use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize)]
pub struct ShopStatus {
    pub is_open: bool,
    pub toggled_at_ms: u64,
}

impl Default for ShopStatus {
    fn default() -> Self {
        Self {
            is_open: true,
            toggled_at_ms: 0,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn shop_status_path(workspace: &Path) -> std::path::PathBuf {
    workspace.join("shop_status.json")
}

pub fn load_shop_status(workspace: &Path) -> anyhow::Result<ShopStatus> {
    let path = shop_status_path(workspace);
    if !path.exists() {
        return Ok(ShopStatus::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    let status: ShopStatus = serde_json::from_str(&raw)?;
    Ok(status)
}

pub fn save_shop_status(workspace: &Path, status: &ShopStatus) -> anyhow::Result<()> {
    let path = shop_status_path(workspace);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(status)?)?;
    Ok(())
}

pub fn toggle_shop_status(workspace: &Path) -> anyhow::Result<ShopStatus> {
    let mut status = load_shop_status(workspace)?;
    status.is_open = !status.is_open;
    status.toggled_at_ms = now_ms();
    save_shop_status(workspace, &status)?;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-shop-status-{unique}"))
    }

    #[test]
    fn default_status_is_open() {
        let status = ShopStatus::default();
        assert!(status.is_open);
    }

    #[test]
    fn toggle_switches_status() {
        let workspace = temp_workspace();
        fs::create_dir_all(&workspace).unwrap();

        // Initially open
        let status = load_shop_status(&workspace).unwrap();
        assert!(status.is_open);

        // Toggle to closed
        let status = toggle_shop_status(&workspace).unwrap();
        assert!(!status.is_open);
        assert!(status.toggled_at_ms > 0);

        // Toggle back to open
        let status = toggle_shop_status(&workspace).unwrap();
        assert!(status.is_open);

        let _ = fs::remove_dir_all(&workspace);
    }
}
