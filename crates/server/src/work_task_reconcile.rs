use crate::{
    employee::list_employee_records,
    requirement::{list_requirement_summaries, RequirementSummary},
    requirement_development,
    requirement_review,
};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct WorkTaskReconcileReport {
    pub requirements_processed: usize,
    pub development_reconciled: usize,
    pub review_reconciled: usize,
    pub errors: Vec<String>,
}

pub fn reconcile_workspace_work_tasks(workspace: &Path) -> WorkTaskReconcileReport {
    let mut report = WorkTaskReconcileReport::default();
    let employees = match list_employee_records(workspace) {
        Ok(items) => items,
        Err(err) => {
            report.errors.push(err.to_string());
            return report;
        }
    };

    let summaries = match list_requirement_summaries(workspace) {
        Ok(items) => items,
        Err(err) => {
            report.errors.push(err.to_string());
            return report;
        }
    };

    for item in summaries {
        report.requirements_processed += 1;
        reconcile_requirement(workspace, &item, &employees, &mut report);
    }
    report
}

fn reconcile_requirement(
    workspace: &Path,
    item: &RequirementSummary,
    employees: &[crate::employee::EmployeeRecord],
    report: &mut WorkTaskReconcileReport,
) {
    match requirement_development::reconcile_development_work_tasks(workspace, &item.id) {
        Ok(()) => report.development_reconciled += 1,
        Err(err) => report.errors.push(format!("development/{}: {err}", item.id)),
    }

    match requirement_review::reconcile_review_work_tasks(workspace, &item.id, employees) {
        Ok(()) => report.review_reconciled += 1,
        Err(err) => report.errors.push(format!("review/{}: {err}", item.id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requirement::{format_requirement_md, RequirementMeta, RequirementPhase, REQUIREMENT_FILE};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn reconcile_completed_review_marks_tasks_completed() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("kaisha-reconcile-{unique}"));
        fs::create_dir_all(&workspace).unwrap();
        let employee_dir = crate::employee::employee_root(&workspace).join("alice");
        fs::create_dir_all(&employee_dir).unwrap();
        fs::write(
            employee_dir.join("profile.json"),
            serde_json::json!({
                "id": "alice",
                "name": "Alice",
                "department": "engineering",
                "role": "Engineer"
            })
            .to_string(),
        )
        .unwrap();
        let req_dir = workspace.join("requirements").join("auth");
        fs::create_dir_all(req_dir.join("review")).unwrap();
        fs::write(
            req_dir.join(REQUIREMENT_FILE),
            format_requirement_md(
                &RequirementMeta {
                    id: "auth".into(),
                    title: "User auth".into(),
                    phase: RequirementPhase::Collection,
                    created_at_ms: 1,
                    updated_at_ms: 2,
                },
                "## Scope",
            ),
        )
        .unwrap();
        fs::write(
            req_dir.join("review/state.json"),
            serde_json::json!({
                "requirement_id": "auth",
                "status": "completed",
                "started_at_ms": 1,
                "completed_at_ms": 2,
                "conclusion": "adopt",
                "participants": ["alice"]
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            req_dir.join("review/summary.md"),
            "CONCLUSION: adopt\n",
        )
        .unwrap();

        let report = reconcile_workspace_work_tasks(&workspace);
        assert_eq!(report.review_reconciled, 1);
        let tasks = crate::work_task::list_work_tasks_filtered(
            &workspace,
            &crate::work_task::WorkTaskFilter {
                biz_id: Some("auth".into()),
                task_kind: Some(crate::work_task::TASK_KIND_REVIEW.into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, crate::work_task::WorkTaskStatus::Completed);
        assert_eq!(
            crate::work_task::review_passed(&tasks[0]),
            Some(true)
        );
        let _ = fs::remove_dir_all(&workspace);
    }
}
