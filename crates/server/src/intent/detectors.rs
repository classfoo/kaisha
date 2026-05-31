use crate::employee_intent_router::{IntentContext, IntentDetection, IntentDetector, IntentType, extract_requirement_id};

/// Detects requirement review intent.
/// Priority: 100 (highest - most specific action)
pub struct ReviewIntentDetector;

impl IntentDetector for ReviewIntentDetector {
    fn detect(&self, input: &str, context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "进入需求评审",
            "开始需求评审",
            "启动需求评审",
            "需求评审",
            "开始评审",
            "进入评审",
            "start requirement review",
            "enter requirement review",
            "run requirement review",
            "开始评审需求",
            "发起评审",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            let req_id = context.find_requirement_id(input)
                .or_else(|| extract_requirement_id(input, &context.known_requirement_ids))?;
            let mut params = std::collections::HashMap::new();
            params.insert("requirement_id".to_string(), req_id);
            return Some(IntentDetection {
                intent_type: IntentType::RequirementReview,
                confidence: 0.9,
                params,
            });
        }
        None
    }

    fn priority(&self) -> i32 { 100 }
}

/// Detects requirement list intent.
/// Priority: 90
pub struct RequirementListDetector;

impl IntentDetector for RequirementListDetector {
    fn detect(&self, input: &str, _context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "列出需求",
            "所有需求",
            "需求列表",
            "查看需求",
            "显示需求",
            "list requirements",
            "show requirements",
            "all requirements",
            "requirement list",
            "查看有哪些需求",
            "需求有哪些",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            return Some(IntentDetection {
                intent_type: IntentType::RequirementList,
                confidence: 0.9,
                params: std::collections::HashMap::new(),
            });
        }
        None
    }

    fn priority(&self) -> i32 { 90 }
}

/// Detects requirement detail intent.
/// Priority: 90
pub struct RequirementDetailDetector;

impl IntentDetector for RequirementDetailDetector {
    fn detect(&self, input: &str, context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "查看需求详情",
            "需求详情",
            "看看需求",
            "需求详情",
            "requirement detail",
            "show requirement",
            "查看需求",
            "看下需求",
            "查看详情",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            let req_id = context.find_requirement_id(input)
                .or_else(|| extract_requirement_id(input, &context.known_requirement_ids))?;
            let mut params = std::collections::HashMap::new();
            params.insert("requirement_id".to_string(), req_id);
            return Some(IntentDetection {
                intent_type: IntentType::RequirementDetail,
                confidence: 0.85,
                params,
            });
        }
        None
    }

    fn priority(&self) -> i32 { 90 }
}

/// Detects requirement confirm intent.
/// Priority: 90
pub struct RequirementConfirmDetector;

impl IntentDetector for RequirementConfirmDetector {
    fn detect(&self, input: &str, context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "确认需求",
            "需求确认",
            "确认这个需求",
            "confirm requirement",
            "confirm requirement",
            "需求通过",
            "通过需求",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            let req_id = context.find_requirement_id(input)
                .or_else(|| extract_requirement_id(input, &context.known_requirement_ids))?;
            let mut params = std::collections::HashMap::new();
            params.insert("requirement_id".to_string(), req_id);
            return Some(IntentDetection {
                intent_type: IntentType::RequirementConfirm,
                confidence: 0.85,
                params,
            });
        }
        None
    }

    fn priority(&self) -> i32 { 90 }
}

/// Detects development start intent.
/// Priority: 80
pub struct DevelopmentStartDetector;

impl IntentDetector for DevelopmentStartDetector {
    fn detect(&self, input: &str, context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "开始开发",
            "启动开发",
            "开始编码",
            "进入开发",
            "start development",
            "start coding",
            "begin development",
            "开始写代码",
            "开发这个需求",
            "开始做",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            let req_id = context.find_requirement_id(input)
                .or_else(|| extract_requirement_id(input, &context.known_requirement_ids));
            let mut params = std::collections::HashMap::new();
            if let Some(id) = req_id {
                params.insert("requirement_id".to_string(), id);
            }
            return Some(IntentDetection {
                intent_type: IntentType::DevelopmentStart,
                confidence: 0.85,
                params,
            });
        }
        None
    }

    fn priority(&self) -> i32 { 80 }
}

/// Detects dev task creation intent.
/// Priority: 80
pub struct DevTaskCreateDetector;

impl IntentDetector for DevTaskCreateDetector {
    fn detect(&self, input: &str, context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "创建任务",
            "新建任务",
            "添加任务",
            "创建开发任务",
            "create task",
            "add task",
            "new task",
            "创建一个任务",
            "新建一个开发任务",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            let req_id = context.find_requirement_id(input)
                .or_else(|| extract_requirement_id(input, &context.known_requirement_ids));
            let mut params = std::collections::HashMap::new();
            if let Some(id) = req_id {
                params.insert("requirement_id".to_string(), id);
            }
            return Some(IntentDetection {
                intent_type: IntentType::DevTaskCreate,
                confidence: 0.85,
                params,
            });
        }
        None
    }

    fn priority(&self) -> i32 { 80 }
}

/// Detects dev task list intent.
/// Priority: 80
pub struct DevTaskListDetector;

impl IntentDetector for DevTaskListDetector {
    fn detect(&self, input: &str, context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "任务列表",
            "开发任务列表",
            "查看任务",
            "显示任务",
            "task list",
            "show tasks",
            "list tasks",
            "有哪些任务",
            "查看开发任务",
            "开发任务有哪些",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            let req_id = context.find_requirement_id(input)
                .or_else(|| extract_requirement_id(input, &context.known_requirement_ids));
            let mut params = std::collections::HashMap::new();
            if let Some(id) = req_id {
                params.insert("requirement_id".to_string(), id);
            }
            return Some(IntentDetection {
                intent_type: IntentType::DevTaskList,
                confidence: 0.85,
                params,
            });
        }
        None
    }

    fn priority(&self) -> i32 { 80 }
}

/// Detects work task list intent.
/// Priority: 70
pub struct WorkTaskListDetector;

impl IntentDetector for WorkTaskListDetector {
    fn detect(&self, input: &str, context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "我的任务",
            "工作任务",
            "任务列表",
            "查看我的任务",
            "my tasks",
            "work tasks",
            "show my tasks",
            "我有什么任务",
            "分配给我的任务",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            let mut params = std::collections::HashMap::new();
            params.insert("employee_id".to_string(), context.employee_id.to_string());
            return Some(IntentDetection {
                intent_type: IntentType::WorkTaskList,
                confidence: 0.8,
                params,
            });
        }
        None
    }

    fn priority(&self) -> i32 { 70 }
}

/// Detects autonomy run intent.
/// Priority: 60
pub struct AutonomyRunDetector;

impl IntentDetector for AutonomyRunDetector {
    fn detect(&self, input: &str, _context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "自主运行",
            "开始工作",
            "自主工作",
            "run autonomy",
            "start autonomy",
            "autonomous work",
            "自己工作",
            "自主模式",
            "开始自主",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            return Some(IntentDetection {
                intent_type: IntentType::AutonomyRun,
                confidence: 0.8,
                params: std::collections::HashMap::new(),
            });
        }
        None
    }

    fn priority(&self) -> i32 { 60 }
}

/// Detects autonomy explore intent.
/// Priority: 60
pub struct AutonomyExploreDetector;

impl IntentDetector for AutonomyExploreDetector {
    fn detect(&self, input: &str, _context: &IntentContext) -> Option<IntentDetection> {
        let lower = input.to_lowercase();
        let triggers = [
            "探索代码",
            "探索",
            "explore code",
            "start explore",
            "探索模式",
            "分析代码",
            "explore",
            "开始探索",
        ];

        if triggers.iter().any(|t| lower.contains(t)) {
            return Some(IntentDetection {
                intent_type: IntentType::AutonomyExplore,
                confidence: 0.8,
                params: std::collections::HashMap::new(),
            });
        }
        None
    }

    fn priority(&self) -> i32 { 60 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn empty_context() -> IntentContext<'static> {
        IntentContext {
            workspace: Path::new("/tmp"),
            employee_id: "emp-1",
            known_requirement_ids: vec!["req-001".to_string(), "auth".to_string()],
            known_employee_ids: vec!["alice".to_string(), "bob".to_string()],
        }
    }

    #[test]
    fn review_detector_matches_trigger() {
        let detector = ReviewIntentDetector;
        let ctx = empty_context();

        let result = detector.detect("对 auth 开始评审", &ctx);
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().intent_type, IntentType::RequirementReview);
        assert_eq!(result.as_ref().unwrap().params.get("requirement_id"), Some(&"auth".to_string()));
    }

    #[test]
    fn requirement_list_detector_matches() {
        let detector = RequirementListDetector;
        let ctx = empty_context();

        let result = detector.detect("列出所有需求", &ctx);
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().intent_type, IntentType::RequirementList);
    }

    #[test]
    fn development_start_detector_matches() {
        let detector = DevelopmentStartDetector;
        let ctx = empty_context();

        let result = detector.detect("开始开发 req-001", &ctx);
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().intent_type, IntentType::DevelopmentStart);
    }

    #[test]
    fn dev_task_create_detector_matches() {
        let detector = DevTaskCreateDetector;
        let ctx = empty_context();

        let result = detector.detect("创建一个任务", &ctx);
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().intent_type, IntentType::DevTaskCreate);
    }

    #[test]
    fn autonomy_run_detector_matches() {
        let detector = AutonomyRunDetector;
        let ctx = empty_context();

        let result = detector.detect("自主运行", &ctx);
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().intent_type, IntentType::AutonomyRun);
    }

    #[test]
    fn detectors_return_none_for_non_matching_input() {
        let ctx = empty_context();

        assert!(ReviewIntentDetector.detect("随便聊聊", &ctx).is_none());
        assert!(RequirementListDetector.detect("今天天气不错", &ctx).is_none());
        assert!(DevelopmentStartDetector.detect("吃了吗", &ctx).is_none());
    }
}
