#![allow(dead_code)]

use crate::tools::model::{ToolFormSchema, ToolKind};
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ToolSession {
    pub id: String,
    pub started_at_ms: u128,
}

#[derive(Debug, Clone)]
pub struct ToolChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ToolUsage {
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub output: String,
    pub exit_code: i32,
    pub usage: ToolUsage,
}

/// Shell-invoked chat process (program + args + extra env). Working directory is applied by the runner.
#[derive(Debug, Clone)]
pub struct ChatSubprocessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

pub fn join_chat_prompt(messages: &[ToolChatMessage]) -> String {
    messages
        .iter()
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn merge_shell_output(stdout: &str, stderr: &str) -> String {
    if stderr.trim().is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n\n--- stderr ---\n{stderr}")
    }
}

pub trait CodingToolDriver: Send + Sync {
    fn kind(&self) -> ToolKind;
    fn display_name(&self) -> &'static str;
    fn schema(&self) -> ToolFormSchema;
    fn default_config(&self) -> Value;
    fn validate(&self, config: &Value) -> anyhow::Result<()>;

    /// Build the subprocess used for `run_chat_for_code` and streaming execution.
    fn chat_subprocess_spec(&self, config: &Value, messages: &[ToolChatMessage]) -> anyhow::Result<ChatSubprocessSpec>;

    // 1) Capability: check whether tool binary exists.
    fn check_installed(&self, config: &Value) -> anyhow::Result<bool> {
        let command = resolve_command(config, &self.default_config())?;
        Ok(command_exists(&command))
    }

    // 2) Capability: install tool.
    fn install_tool(&self, _config: &Value) -> anyhow::Result<()> {
        anyhow::bail!(
            "install is not implemented for {}; install manually or override this driver method",
            self.display_name()
        )
    }

    // 3) Capability: configure tool.
    fn configure_tool(&self, config: &Value) -> anyhow::Result<Value> {
        self.validate(config)?;
        Ok(config.clone())
    }

    // 4) Capability: start tool with config.
    fn start_tool(&self, config: &Value) -> anyhow::Result<()> {
        self.validate(config)?;
        Ok(())
    }

    // 5) Capability: session management.
    fn create_session(&self, config: &Value) -> anyhow::Result<ToolSession> {
        self.start_tool(config)?;
        let now = now_ms()?;
        Ok(ToolSession {
            id: format!("{}_session_{}", self.kind_id(), now),
            started_at_ms: now,
        })
    }

    // 6) Capability: dialogue/code execution (blocking; uses `chat_subprocess_spec`).
    fn run_chat_for_code(
        &self,
        config: &Value,
        _session: &ToolSession,
        messages: &[ToolChatMessage],
        cwd: Option<&Path>,
    ) -> anyhow::Result<ToolExecutionResult> {
        self.validate(config)?;
        let spec = self.chat_subprocess_spec(config, messages)?;
        let mut cmd = Command::new(&spec.program);
        for a in &spec.args {
            cmd.arg(a);
        }
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let output = cmd.output().map_err(|e| anyhow::anyhow!("{}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let merged = merge_shell_output(&stdout, &stderr);
        let usage = self.collect_usage(config, messages, &merged)?;
        Ok(ToolExecutionResult {
            output: merged,
            exit_code: output.status.code().unwrap_or(1),
            usage,
        })
    }

    // 7) Capability: token usage stats.
    fn collect_usage(
        &self,
        config: &Value,
        messages: &[ToolChatMessage],
        completion: &str,
    ) -> anyhow::Result<ToolUsage> {
        let prompt_tokens = estimate_tokens(messages);
        let completion_tokens = ((completion.chars().count() as f64) / 4.0).ceil() as u64;
        Ok(ToolUsage {
            model: config
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        })
    }

    fn kind_id(&self) -> String {
        format!("{:?}", self.kind()).to_lowercase()
    }
}

fn now_ms() -> anyhow::Result<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn resolve_command(config: &Value, defaults: &Value) -> anyhow::Result<String> {
    if let Some(cmd) = config.get("command").and_then(Value::as_str) {
        if !cmd.trim().is_empty() {
            return Ok(cmd.trim().to_string());
        }
    }
    if let Some(cmd) = defaults.get("command").and_then(Value::as_str) {
        if !cmd.trim().is_empty() {
            return Ok(cmd.trim().to_string());
        }
    }
    anyhow::bail!("missing command in tool config")
}

fn command_exists(command: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", command))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn estimate_tokens(messages: &[ToolChatMessage]) -> u64 {
    let chars: usize = messages
        .iter()
        .map(|m| m.role.chars().count() + m.content.chars().count())
        .sum();
    ((chars as f64) / 4.0).ceil() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_chat_prompt_formats_roles() {
        let messages = vec![
            ToolChatMessage {
                role: "system".into(),
                content: "rules".into(),
            },
            ToolChatMessage {
                role: "user".into(),
                content: "hello".into(),
            },
        ];
        assert_eq!(join_chat_prompt(&messages), "system: rules\nuser: hello");
    }

    #[test]
    fn merge_shell_output_omits_stderr_when_empty() {
        assert_eq!(merge_shell_output("out", "  \n"), "out");
    }

    #[test]
    fn merge_shell_output_appends_stderr_section() {
        let merged = merge_shell_output("out", "warn");
        assert!(merged.contains("out"));
        assert!(merged.contains("--- stderr ---"));
        assert!(merged.contains("warn"));
    }
}
