use std::path::Path;

use crate::employee::list_employee_records;
use crate::requirement::list_requirement_summaries;

/// Context available during intent detection and handling.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IntentContext<'a> {
    pub workspace: &'a Path,
    pub employee_id: &'a str,
    pub known_requirement_ids: Vec<String>,
    pub known_employee_ids: Vec<String>,
}

impl<'a> IntentContext<'a> {
    #[allow(dead_code)]
    pub fn new(
        workspace: &'a Path,
        employee_id: &'a str,
    ) -> anyhow::Result<Self> {
        let req_summaries = list_requirement_summaries(workspace)?;
        let known_requirement_ids: Vec<String> = req_summaries.iter().map(|s| s.id.clone()).collect();
        let employees = list_employee_records(workspace)?;
        let known_employee_ids: Vec<String> = employees.iter().map(|e| e.id.clone()).collect();
        Ok(Self {
            workspace,
            employee_id,
            known_requirement_ids,
            known_employee_ids,
        })
    }

    pub fn find_requirement_id(&self, input: &str) -> Option<String> {
        for id in &self.known_requirement_ids {
            if input.contains(id.as_str()) {
                return Some(id.clone());
            }
        }
        if self.known_requirement_ids.len() == 1 {
            return self.known_requirement_ids.first().cloned();
        }
        None
    }

    #[allow(dead_code)]
    pub fn find_employee_id(&self, input: &str) -> Option<String> {
        for id in &self.known_employee_ids {
            if input.contains(id.as_str()) {
                return Some(id.clone());
            }
        }
        if self.known_employee_ids.len() == 1 {
            return self.known_employee_ids.first().cloned();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-intent-ctx-{unique}"))
    }

    #[test]
    fn find_requirement_id_matches_known_id() {
        let workspace = temp_workspace();
        let req_dir = workspace.join("requirements").join("auth");
        fs::create_dir_all(&req_dir).unwrap();
        let meta = crate::requirement::RequirementMeta {
            id: "auth".into(),
            title: "Auth".into(),
            phase: crate::requirement::RequirementPhase::Collection,
            created_at_ms: 1,
            updated_at_ms: 2,
        };
        fs::write(
            req_dir.join(crate::requirement::REQUIREMENT_FILE),
            crate::requirement::format_requirement_md(&meta, "## Scope"),
        )
        .unwrap();

        let emp_dir = crate::employee::employee_root(&workspace).join("alice");
        fs::create_dir_all(&emp_dir).unwrap();
        fs::write(
            emp_dir.join("profile.json"),
            serde_json::json!({
                "id": "alice",
                "name": "Alice",
                "department": "engineering",
                "role": "Engineer"
            })
            .to_string(),
        )
        .unwrap();

        let ctx = IntentContext::new(&workspace, "alice").unwrap();
        let result = ctx.find_requirement_id("对 auth 进行评审");
        assert_eq!(result, Some("auth".to_string()));

        let _ = fs::remove_dir_all(&workspace);
    }
}
