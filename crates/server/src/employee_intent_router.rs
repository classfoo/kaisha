use std::collections::HashMap;
use std::sync::Arc;

use crate::tools::manager::ToolManager;
use crate::tools::driver::ToolExecutionResult;

// Re-export IntentContext from intent::context
pub use crate::intent::context::IntentContext;

/// Intent types supported by the router.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntentType {
    /// Trigger requirement review for a specific requirement
    RequirementReview,
    /// List all requirements
    RequirementList,
    /// Show detail for a specific requirement
    RequirementDetail,
    /// Confirm a requirement
    RequirementConfirm,
    /// Start development for a requirement
    DevelopmentStart,
    /// Create a development task
    DevTaskCreate,
    /// List development tasks
    DevTaskList,
    /// List work tasks
    WorkTaskList,
    /// Show work task detail
    WorkTaskDetail,
    /// Assign work task
    WorkTaskAssign,
    /// Run employee autonomy
    AutonomyRun,
    /// Run autonomy explore
    AutonomyExplore,
    /// Default fallback - general requirement management
    GeneralChat,
}

impl IntentType {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            IntentType::RequirementReview
                | IntentType::RequirementList
                | IntentType::RequirementDetail
                | IntentType::RequirementConfirm
        )
    }

    pub fn requires_code_agent(&self) -> bool {
        !self.is_terminal()
    }
}

/// Result of intent detection.
#[derive(Debug, Clone)]
pub struct IntentDetection {
    pub intent_type: IntentType,
    pub confidence: f32,
    pub params: HashMap<String, String>,
}

/// Trait for intent detectors.
pub trait IntentDetector: Send + Sync {
    fn detect(&self, input: &str, context: &IntentContext) -> Option<IntentDetection>;
    fn priority(&self) -> i32;
}

/// Result from intent handler execution.
#[derive(Debug)]
pub struct IntentResult {
    pub output: String,
    pub execution_result: Option<ToolExecutionResult>,
    pub tool_instance_id: Option<String>,
    pub task_id: Option<String>,
    pub output_preview: Option<String>,
}

/// Trait for intent handlers.
pub trait IntentHandler: Send + Sync {
    fn handle(
        &self,
        intent: &IntentDetection,
        tools: &ToolManager,
        workspace: &std::path::Path,
        employee_id: &str,
        user_input: &str,
        prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String>;
}

/// Router that manages intent detection and handler dispatch.
pub struct IntentRouter {
    pub detectors: Vec<Arc<dyn IntentDetector>>,
    pub handlers: HashMap<IntentType, Arc<dyn IntentHandler>>,
    pub default_handler: Arc<dyn IntentHandler>,
}

impl IntentRouter {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            handlers: HashMap::new(),
            default_handler: Arc::new(crate::intent::handlers::GeneralChatHandler),
        }
    }

    pub fn register_detector(&mut self, detector: Arc<dyn IntentDetector>) {
        self.detectors.push(detector);
        self.detectors.sort_by_key(|d| -(d.priority()));
    }

    pub fn register_handler(&mut self, intent_type: IntentType, handler: Arc<dyn IntentHandler>) {
        self.handlers.insert(intent_type, handler);
    }

    pub fn detect(&self, input: &str, context: &IntentContext) -> Option<IntentDetection> {
        for detector in &self.detectors {
            if let Some(detection) = detector.detect(input, context) {
                if detection.confidence > 0.5 {
                    return Some(detection);
                }
            }
        }
        None
    }

    pub fn route_and_handle(
        &self,
        input: &str,
        context: &IntentContext,
        tools: &ToolManager,
        workspace: &std::path::Path,
        employee_id: &str,
        prior_messages: &[(String, String)],
    ) -> Result<IntentResult, String> {
        if let Some(detection) = self.detect(input, context) {
            if let Some(handler) = self.handlers.get(&detection.intent_type) {
                return handler.handle(&detection, tools, workspace, employee_id, input, prior_messages);
            }
        }
        self.default_handler.handle(
            &IntentDetection {
                intent_type: IntentType::GeneralChat,
                confidence: 1.0,
                params: HashMap::new(),
            },
            tools,
            workspace,
            employee_id,
            input,
            prior_messages,
        )
    }

    pub fn detect_intent_type(&self, input: &str, context: &IntentContext) -> Option<IntentType> {
        self.detect(input, context).map(|d| d.intent_type)
    }
}

/// Helper to extract a requirement ID from input text.
pub fn extract_requirement_id(input: &str, known_ids: &[String]) -> Option<String> {
    for id in known_ids {
        if input.contains(id.as_str()) {
            return Some(id.clone());
        }
    }
    known_ids.first().cloned()
}

/// Helper to extract an employee ID from input text.
pub fn extract_employee_id(input: &str, known_ids: &[String]) -> Option<String> {
    for id in known_ids {
        if input.contains(id.as_str()) {
            return Some(id.clone());
        }
    }
    known_ids.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::context::IntentContext;
    use std::path::Path;

    #[test]
    fn extract_requirement_id_finds_known_id() {
        let ids = vec!["req-001".to_string(), "req-002".to_string()];
        let result = extract_requirement_id("对 req-001 开始评审", &ids);
        assert_eq!(result, Some("req-001".to_string()));
    }

    #[test]
    fn extract_requirement_id_returns_first_when_no_match() {
        let ids = vec!["req-001".to_string()];
        let result = extract_requirement_id("开始评审", &ids);
        assert_eq!(result, Some("req-001".to_string()));
    }

    #[test]
    fn extract_requirement_id_returns_none_for_empty_ids() {
        let ids: Vec<String> = vec![];
        let result = extract_requirement_id("开始评审", &ids);
        assert!(result.is_none());
    }

    #[test]
    fn intent_router_detects_with_confidence() {
        struct TestDetector;
        impl IntentDetector for TestDetector {
            fn detect(&self, input: &str, _context: &IntentContext) -> Option<IntentDetection> {
                if input.contains("test_trigger") {
                    Some(IntentDetection {
                        intent_type: IntentType::RequirementReview,
                        confidence: 0.9,
                        params: HashMap::new(),
                    })
                } else {
                    None
                }
            }
            fn priority(&self) -> i32 { 100 }
        }

        let ctx = IntentContext {
            workspace: Path::new("/tmp"),
            employee_id: "emp-1",
            known_requirement_ids: vec![],
            known_employee_ids: vec![],
        };

        let mut router = IntentRouter::new();
        router.register_detector(Arc::new(TestDetector));

        let detection = router.detect("test_trigger now", &ctx);
        assert!(detection.is_some());
        assert_eq!(detection.unwrap().intent_type, IntentType::RequirementReview);

        let no_match = router.detect("random text", &ctx);
        assert!(no_match.is_none());
    }
}
