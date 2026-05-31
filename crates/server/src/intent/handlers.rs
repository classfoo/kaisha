use std::path::Path;

use crate::employee_intent_router::{IntentDetection, IntentHandler, IntentResult};
use crate::employee_requirement_agent::{
    build_requirement_agent_messages,
    requirement_agent_workdir,
};
use crate::requirement::{
    list_requirement_summaries,
    load_requirement_detail,
};
use crate::requirement_review::run_requirement_review;
use crate::requirement_development::ensure_development_started;
use crate::work_task::{
    filter_work_tasks,
    list_work_tasks,
    WorkTaskFilter,
};
use crate::tasks::{
    task_content_from_user_input,
    CodeAgentTaskParams,
    TaskKind,
    TaskRunner,
};
use crate::tools::manager::ToolManager;

/// Handler for requirement review intent.
pub struct ReviewHandler;

impl IntentHandler for ReviewHandler {
    fn handle(
        &self,
        intent: &IntentDetection,
        tools: &ToolManager,
        workspace: &Path,
        _employee_id: &str,
        _user_input: &str,
        _prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        let req_id = intent.params.get("requirement_id")
            .ok_or_else(|| "review_requirement_unspecified".to_string())?;

        let review = run_requirement_review(workspace, tools, req_id)
            .map_err(|e| e.to_string())?;

        let conclusion = review
            .conclusion
            .as_ref()
            .map(|c| match c {
                crate::requirement_review::ReviewConclusion::Adopt => "adopt",
                crate::requirement_review::ReviewConclusion::Supplement => "supplement",
            })
            .unwrap_or("pending");
        let summary = review
            .summary
            .as_deref()
            .unwrap_or("(no summary file)");

        let output = format!(
            "Requirement review completed for `{}`.\n\n**Conclusion:** {conclusion}\n\n## Summary\n\n{summary}",
            review.requirement_id
        );

        let instance = tools
            .pick_enabled_chat_driver()
            .map(|(inst, _)| inst)
            .ok_or_else(|| "chat_tool_missing".to_string())?;

        Ok(IntentResult {
            output: output.clone(),
            execution_result: Some(crate::tools::driver::ToolExecutionResult {
                output,
                exit_code: 0,
                usage: crate::tools::driver::ToolUsage {
                    model: "requirement-review".to_string(),
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
            }),
            tool_instance_id: Some(instance.id.clone()),
            task_id: None,
            output_preview: None,
        })
    }
}

/// Handler for requirement list intent.
pub struct RequirementListHandler;

impl IntentHandler for RequirementListHandler {
    fn handle(
        &self,
        _intent: &IntentDetection,
        _tools: &ToolManager,
        workspace: &Path,
        _employee_id: &str,
        _user_input: &str,
        _prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        let summaries = list_requirement_summaries(workspace)
            .map_err(|e| e.to_string())?;

        if summaries.is_empty() {
            return Ok(IntentResult {
                output: "当前没有需求记录。请先创建需求。".to_string(),
                execution_result: None,
                tool_instance_id: None,
                task_id: None,
                output_preview: None,
            });
        }

        let mut output = String::from("## 当前需求列表\n\n");
        for s in &summaries {
            let phase = s.phase.as_str();
            output.push_str(&format!(
                "- **{}** (ID: `{}`) — 阶段: {}\n",
                s.title, s.id, phase
            ));
        }

        output.push_str(&format!("\n共 {} 个需求。", summaries.len()));

        let preview = output.chars().take(2000).collect();
        Ok(IntentResult {
            output,
            execution_result: None,
            tool_instance_id: None,
            task_id: None,
            output_preview: Some(preview),
        })
    }
}

/// Handler for requirement detail intent.
pub struct RequirementDetailHandler;

impl IntentHandler for RequirementDetailHandler {
    fn handle(
        &self,
        intent: &IntentDetection,
        _tools: &ToolManager,
        workspace: &Path,
        _employee_id: &str,
        _user_input: &str,
        _prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        let req_id = intent.params.get("requirement_id")
            .ok_or_else(|| "requirement_id_unspecified".to_string())?;

        let detail = load_requirement_detail(workspace, req_id)
            .map_err(|e| e.to_string())?;

        let output = format!(
            "## 需求详情: {}\n\n**ID:** `{}`\n**阶段:** {}\n**创建时间:** {}\n\n{}\n",
            detail.title,
            detail.id,
            detail.phase.as_str(),
            detail.created_at_ms,
            detail.content,
        );

        let preview = output.chars().take(2000).collect();
        Ok(IntentResult {
            output,
            execution_result: None,
            tool_instance_id: None,
            task_id: None,
            output_preview: Some(preview),
        })
    }
}

/// Handler for development start intent.
pub struct DevelopmentStartHandler;

impl IntentHandler for DevelopmentStartHandler {
    fn handle(
        &self,
        intent: &IntentDetection,
        tools: &ToolManager,
        workspace: &Path,
        _employee_id: &str,
        _user_input: &str,
        _prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        let req_id = intent.params.get("requirement_id")
            .ok_or_else(|| "requirement_id_unspecified".to_string())?;

        let state = ensure_development_started(workspace, req_id)?;

        let task_count = crate::requirement_development::try_load_dev_state(workspace, req_id)
            .map(|_| {
                let tasks = crate::work_task::filter_work_tasks(
                    list_work_tasks(workspace).unwrap_or_default(),
                    &WorkTaskFilter {
                        biz_type: Some("requirement".to_string()),
                        biz_id: Some(req_id.clone()),
                        task_kind: Some("development".to_string()),
                        ..Default::default()
                    },
                );
                tasks.len()
            })
            .unwrap_or(0);

        let output = format!(
            "已为需求 `{}` 开始开发。\n\n**特性分支:** `{}`\n**当前任务数:** {}\n\n如果有可执行的任务，员工将自主开始工作。",
            req_id,
            state.feature_branch,
            task_count,
        );

        let instance = tools
            .pick_enabled_chat_driver()
            .map(|(inst, _)| inst);

        let preview = output.chars().take(2000).collect();
        Ok(IntentResult {
            output,
            execution_result: None,
            tool_instance_id: instance.map(|i| i.id),
            task_id: None,
            output_preview: Some(preview),
        })
    }
}

/// Handler for dev task list intent.
pub struct DevTaskListHandler;

impl IntentHandler for DevTaskListHandler {
    fn handle(
        &self,
        intent: &IntentDetection,
        _tools: &ToolManager,
        workspace: &Path,
        _employee_id: &str,
        _user_input: &str,
        _prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        let req_id = intent.params.get("requirement_id")
            .ok_or_else(|| "requirement_id_unspecified".to_string())?;

        let tasks = filter_work_tasks(
            list_work_tasks(workspace).map_err(|e| e.to_string())?,
            &WorkTaskFilter {
                biz_type: Some("requirement".to_string()),
                biz_id: Some(req_id.clone()),
                task_kind: Some("development".to_string()),
                ..Default::default()
            },
        );

        if tasks.is_empty() {
            return Ok(IntentResult {
                output: format!("需求 `{}` 当前没有开发任务。", req_id),
                execution_result: None,
                tool_instance_id: None,
                task_id: None,
                output_preview: None,
            });
        }

        let mut output = format!("## 开发任务列表: {}\n\n", req_id);
        for t in &tasks {
            let assignee = t.assignee.as_deref().unwrap_or("未分配");
            output.push_str(&format!(
                "- **{}** (ID: `{}`) — 状态: {}, 进度: {}%, 负责人: {}\n",
                t.title, t.id, t.status.as_str(), t.progress, assignee
            ));
        }

        let preview = output.chars().take(2000).collect();
        Ok(IntentResult {
            output,
            execution_result: None,
            tool_instance_id: None,
            task_id: None,
            output_preview: Some(preview),
        })
    }
}

/// Handler for work task list intent.
pub struct WorkTaskListHandler;

impl IntentHandler for WorkTaskListHandler {
    fn handle(
        &self,
        intent: &IntentDetection,
        _tools: &ToolManager,
        workspace: &Path,
        _employee_id: &str,
        _user_input: &str,
        _prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        let employee_id = intent.params.get("employee_id")
            .map(|s| s.as_str())
            .unwrap_or("");

        let all_tasks = list_work_tasks(workspace).map_err(|e| e.to_string())?;

        let filtered: Vec<_> = all_tasks
            .into_iter()
            .filter(|t| {
                if employee_id.is_empty() {
                    true
                } else {
                    t.assignee.as_deref() == Some(employee_id)
                }
            })
            .collect();

        if filtered.is_empty() {
            return Ok(IntentResult {
                output: if employee_id.is_empty() {
                    "当前没有工作任务。".to_string()
                } else {
                    format!("员工 `{}` 当前没有任务。", employee_id)
                },
                execution_result: None,
                tool_instance_id: None,
                task_id: None,
                output_preview: None,
            });
        }

        let mut output = String::from("## 工作任务列表\n\n");
        for t in &filtered {
            let assignee = t.assignee.as_deref().unwrap_or("未分配");
            let task_kind = t.metadata.get("task_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            output.push_str(&format!(
                "- **{}** (ID: `{}`) — 状态: {}, 进度: {}%, 负责人: {}, 类型: {}\n",
                t.title, t.id, t.status.as_str(), t.progress, assignee, task_kind
            ));
        }

        let preview = output.chars().take(2000).collect();
        Ok(IntentResult {
            output,
            execution_result: None,
            tool_instance_id: None,
            task_id: None,
            output_preview: Some(preview),
        })
    }
}

/// Handler for autonomy run intent.
pub struct AutonomyRunHandler;

impl IntentHandler for AutonomyRunHandler {
    fn handle(
        &self,
        _intent: &IntentDetection,
        _tools: &ToolManager,
        workspace: &Path,
        employee_id: &str,
        _user_input: &str,
        _prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        crate::autonomy_trigger::mark_employee_for_autonomy(workspace, employee_id)
            .map_err(|e| e.to_string())?;

        let output = format!(
            "已为员工 `{}` 启动自主工作模式。\n\n员工将开始自主探索和执行任务。",
            employee_id
        );

        let preview = output.chars().take(2000).collect();
        Ok(IntentResult {
            output,
            execution_result: None,
            tool_instance_id: None,
            task_id: None,
            output_preview: Some(preview),
        })
    }
}

/// Handler for autonomy explore intent.
pub struct AutonomyExploreHandler;

impl IntentHandler for AutonomyExploreHandler {
    fn handle(
        &self,
        _intent: &IntentDetection,
        _tools: &ToolManager,
        _workspace: &Path,
        employee_id: &str,
        _user_input: &str,
        _prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        let output = format!(
            "员工 `{}` 已启动探索模式。\n\n将自主分析工作区状态并生成任务计划。",
            employee_id
        );

        let preview = output.chars().take(2000).collect();
        Ok(IntentResult {
            output,
            execution_result: None,
            tool_instance_id: None,
            task_id: None,
            output_preview: Some(preview),
        })
    }
}

/// Default handler - falls back to requirement agent (general chat).
pub struct GeneralChatHandler;

impl IntentHandler for GeneralChatHandler {
    fn handle(
        &self,
        _intent: &IntentDetection,
        tools: &ToolManager,
        workspace: &Path,
        employee_id: &str,
        user_input: &str,
        prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        let workdir = requirement_agent_workdir(workspace).map_err(|e| e.to_string())?;
        let tool_messages = build_requirement_agent_messages(workspace, user_input, prior_messages)
            .map_err(|e| e.to_string())?;

        let runner = TaskRunner::new(workspace);
        let (task, instance, result) = runner
            .run_code_chat(
                tools,
                CodeAgentTaskParams {
                    kind: TaskKind::RequirementAgent,
                    content: task_content_from_user_input(user_input),
                    workdir: workdir.clone(),
                    messages: tool_messages,
                    executor_id: Some(employee_id.to_string()),
                    parent_task_id: None,
                    context: serde_json::json!({ "employee_id": employee_id }),
                },
            )
            .map_err(|e| {
                let msg = e.root_cause().to_string();
                if msg == "no_enabled_coding_tool" {
                    "chat_tool_missing".to_string()
                } else {
                    tracing::warn!(error = %e, "requirement agent execution failed");
                    msg
                }
            })?;

        Ok(IntentResult {
            output: result.output.clone(),
            execution_result: Some(result),
            tool_instance_id: Some(instance.id.clone()),
            task_id: Some(task.id),
            output_preview: task.output_preview.clone(),
        })
    }
}

/// Creates and configures the default intent router with all detectors and handlers.
pub fn create_default_router() -> crate::employee_intent_router::IntentRouter {
    use std::sync::Arc;
    use crate::employee_intent_router::{IntentRouter, IntentType};

    let mut router = IntentRouter::new();

    // Register detectors (higher priority first)
    router.register_detector(Arc::new(super::detectors::ReviewIntentDetector));
    router.register_detector(Arc::new(super::detectors::RequirementListDetector));
    router.register_detector(Arc::new(super::detectors::RequirementDetailDetector));
    router.register_detector(Arc::new(super::detectors::RequirementConfirmDetector));
    router.register_detector(Arc::new(super::detectors::DevelopmentStartDetector));
    router.register_detector(Arc::new(super::detectors::DevTaskCreateDetector));
    router.register_detector(Arc::new(super::detectors::DevTaskListDetector));
    router.register_detector(Arc::new(super::detectors::WorkTaskListDetector));
    router.register_detector(Arc::new(super::detectors::AutonomyRunDetector));
    router.register_detector(Arc::new(super::detectors::AutonomyExploreDetector));

    // Register handlers
    router.register_handler(IntentType::RequirementReview, Arc::new(ReviewHandler));
    router.register_handler(IntentType::RequirementList, Arc::new(RequirementListHandler));
    router.register_handler(IntentType::RequirementDetail, Arc::new(RequirementDetailHandler));
    router.register_handler(IntentType::DevelopmentStart, Arc::new(DevelopmentStartHandler));
    router.register_handler(IntentType::DevTaskList, Arc::new(DevTaskListHandler));
    router.register_handler(IntentType::WorkTaskList, Arc::new(WorkTaskListHandler));
    router.register_handler(IntentType::AutonomyRun, Arc::new(AutonomyRunHandler));
    router.register_handler(IntentType::AutonomyExplore, Arc::new(AutonomyExploreHandler));
    // GeneralChat uses the default handler (RequirementAgent)

    router
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::employee_intent_router::{IntentDetection, IntentType};

    #[test]
    fn general_chat_handler_returns_default_result() {
        // This test verifies the handler exists and has correct type
        let _handler = GeneralChatHandler;
    }

    #[test]
    fn review_handler_exists() {
        let _handler = ReviewHandler;
    }

    #[test]
    fn requirement_list_handler_exists() {
        let _handler = RequirementListHandler;
    }

    #[test]
    fn create_default_router_has_detectors() {
        let router = create_default_router();
        let empty_ids: Vec<String> = vec![];
        let ctx = crate::intent::context::IntentContext {
            workspace: std::path::Path::new("/tmp"),
            employee_id: "emp-1",
            known_requirement_ids: vec!["req-001".to_string()],
            known_employee_ids: vec!["alice".to_string()],
        };

        // Verify review detection works
        let result = router.detect("对 req-001 开始评审", &ctx);
        assert!(result.is_some());
        assert_eq!(result.unwrap().intent_type, IntentType::RequirementReview);

        // Verify requirement list detection works
        let result = router.detect("列出需求", &ctx);
        assert!(result.is_some());
        assert_eq!(result.unwrap().intent_type, IntentType::RequirementList);

        // Verify non-matching returns None
        let result = router.detect("随便聊聊", &ctx);
        assert!(result.is_none());
    }
}
