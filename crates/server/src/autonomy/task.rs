#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Ready,
    Scheduled,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Scheduled => "scheduled",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

impl TaskPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum ComplexityLevel {
    Trivial = 1,
    Simple = 2,
    Moderate = 3,
    Complex = 4,
    VeryComplex = 5,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    Fixed,
    Linear,
    Exponential,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_strategy: BackoffStrategy,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 0,
            backoff_strategy: BackoffStrategy::Exponential,
            initial_delay_ms: 5_000,
            max_delay_ms: 300_000,
        }
    }
}

impl RetryPolicy {
    pub fn next_delay(&self, retry_count: u32) -> u64 {
        match self.backoff_strategy {
            BackoffStrategy::Fixed => self.initial_delay_ms,
            BackoffStrategy::Linear => self.initial_delay_ms * (retry_count as u64 + 1),
            BackoffStrategy::Exponential => {
                let delay = self.initial_delay_ms.saturating_mul(2u64.saturating_pow(retry_count));
                delay.min(self.max_delay_ms)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    #[serde(default)]
    pub requirement_id: Option<String>,
    #[serde(default)]
    pub requirement_phase: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(flatten)]
    pub extra: Value,
}

impl Default for TaskContext {
    fn default() -> Self {
        Self {
            requirement_id: None,
            requirement_phase: None,
            branch: None,
            extra: Value::Object(Default::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub success: bool,
    pub exit_code: i32,
    #[serde(default)]
    pub output_preview: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub agent_task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub complexity: ComplexityLevel,
    pub estimated_duration_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_duration_secs: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_pid: Option<u32>,

    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,

    pub plan_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    pub sub_tasks: Vec<String>,

    pub context: TaskContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskResult>,

    pub retry_policy: RetryPolicy,
    pub retry_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_retry_at_ms: Option<u64>,

    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<u64>,
}

impl Task {
    pub fn is_retryable(&self) -> bool {
        self.retry_count < self.retry_policy.max_retries
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_task_id() -> String {
    format!("task_{}", now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_exponential() {
        let policy = RetryPolicy {
            max_retries: 3,
            backoff_strategy: BackoffStrategy::Exponential,
            initial_delay_ms: 1000,
            max_delay_ms: 10000,
        };
        assert_eq!(policy.next_delay(0), 1000);
        assert_eq!(policy.next_delay(1), 2000);
        assert_eq!(policy.next_delay(2), 4000);
        assert_eq!(policy.next_delay(3), 8000);
        assert_eq!(policy.next_delay(4), 10000); // capped
    }

    #[test]
    fn task_priority_ordering() {
        assert!(TaskPriority::Critical < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Medium);
        assert!(TaskPriority::Medium < TaskPriority::Low);
    }
}
