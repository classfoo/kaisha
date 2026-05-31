use std::path::Path;

use crate::tasks::TaskKind;
use crate::work_task::{create_work_task, CreateWorkTaskParams, WorkTask, WorkTaskStatus};

const BIZ_TYPE_EMPLOYEE: &str = "employee";
const TASK_KIND_AUTONOMY: &str = "autonomy";

/// Creates a WorkTask to track an autonomous agent task.
/// This makes the autonomous work visible in the task list.
pub fn create_work_task_for_autonomy(
    workspace: &Path,
    employee_id: &str,
    agent_task_kind: &TaskKind,
    agent_task_id: &str,
) -> anyhow::Result<WorkTask> {
    let title = format!("自主工作: {}", task_kind_title(agent_task_kind));
    let description = format!(
        "员工自主执行的{}任务\n\nAgent Task ID: {}",
        task_kind_title(agent_task_kind),
        agent_task_id,
    );

    let metadata = serde_json::json!({
        "task_kind": TASK_KIND_AUTONOMY,
        "agent_task_kind": format!("{:?}", agent_task_kind),
        "agent_task_id": agent_task_id,
    });

    create_work_task(
        workspace,
        CreateWorkTaskParams {
            id: None,
            biz_type: BIZ_TYPE_EMPLOYEE,
            biz_id: employee_id,
            title: &title,
            description: &description,
            assignee: Some(employee_id),
            auto_executable: false, // 自主任务不自动执行
            metadata,
        },
    )
    .map_err(|e| anyhow::anyhow!(e))
}

fn task_kind_title(kind: &TaskKind) -> &'static str {
    match kind {
        TaskKind::AutonomyExplore => "探索工作区",
        TaskKind::AutonomyExecute => "执行任务",
        TaskKind::WorkTaskExecute => "执行工作任务",
        _ => "自主工作",
    }
}

/// Updates the associated WorkTask status when agent task status changes.
pub fn sync_work_task_status(
    workspace: &Path,
    agent_task_id: &str,
    agent_status: &crate::tasks::TaskStatus,
) -> anyhow::Result<()> {
    use crate::work_task::{list_work_tasks, update_work_task};

    // Find the work task by agent_task_id
    let tasks = list_work_tasks(workspace).map_err(|e| anyhow::anyhow!(e))?;
    let work_task = tasks.iter().find(|t| {
        t.agent_task_id.as_deref() == Some(agent_task_id)
    });

    if let Some(task) = work_task {
        let work_status = agent_to_work_status(agent_status);
        update_work_task(workspace, &task.id, |t| {
            t.status = work_status;
            Ok(())
        })
        .map_err(|e| anyhow::anyhow!(e))?;
    }

    Ok(())
}

fn agent_to_work_status(agent_status: &crate::tasks::TaskStatus) -> WorkTaskStatus {
    match agent_status {
        crate::tasks::TaskStatus::Pending => WorkTaskStatus::Pending,
        crate::tasks::TaskStatus::Running => WorkTaskStatus::InProgress,
        crate::tasks::TaskStatus::Completed => WorkTaskStatus::Completed,
        crate::tasks::TaskStatus::Failed => WorkTaskStatus::Failed,
        crate::tasks::TaskStatus::Cancelled => WorkTaskStatus::Cancelled,
        _ => WorkTaskStatus::Pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::employee::employee_root;
    use crate::work_task::load_work_task;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("kaisha-autonomy-task-{unique}"))
    }

    #[test]
    fn create_work_task_for_autonomy_creates_task() {
        let workspace = temp_workspace();
        let emp_dir = employee_root(&workspace).join("alice");
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

        let task = create_work_task_for_autonomy(
            &workspace,
            "alice",
            &TaskKind::AutonomyExplore,
            "agent-task-001",
        )
        .unwrap();

        assert_eq!(task.biz_type, BIZ_TYPE_EMPLOYEE);
        assert_eq!(task.biz_id, "alice");
        assert_eq!(task.assignee.as_deref(), Some("alice"));
        assert!(task.title.contains("自主工作"));
        assert!(task.title.contains("探索工作区"));
        assert!(!task.auto_executable);
        assert_eq!(task.metadata["agent_task_id"].as_str(), Some("agent-task-001"));
        assert_eq!(task.metadata["task_kind"].as_str(), Some(TASK_KIND_AUTONOMY));

        // Verify the task can be loaded
        let loaded = load_work_task(&workspace, &task.id).unwrap();
        assert_eq!(loaded.id, task.id);
        assert_eq!(loaded.metadata["agent_task_id"].as_str(), Some("agent-task-001"));

        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn task_kind_title_returns_correct_labels() {
        assert_eq!(
            task_kind_title(&TaskKind::AutonomyExplore),
            "探索工作区"
        );
        assert_eq!(
            task_kind_title(&TaskKind::AutonomyExecute),
            "执行任务"
        );
        assert_eq!(
            task_kind_title(&TaskKind::WorkTaskExecute),
            "执行工作任务"
        );
    }

    #[test]
    fn agent_to_work_status_maps_correctly() {
        assert_eq!(
            agent_to_work_status(&crate::tasks::TaskStatus::Pending),
            WorkTaskStatus::Pending
        );
        assert_eq!(
            agent_to_work_status(&crate::tasks::TaskStatus::Running),
            WorkTaskStatus::InProgress
        );
        assert_eq!(
            agent_to_work_status(&crate::tasks::TaskStatus::Completed),
            WorkTaskStatus::Completed
        );
        assert_eq!(
            agent_to_work_status(&crate::tasks::TaskStatus::Failed),
            WorkTaskStatus::Failed
        );
    }
}
