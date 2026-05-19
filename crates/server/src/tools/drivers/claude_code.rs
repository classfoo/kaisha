use crate::tools::{
    driver::{ChatSubprocessSpec, CodingToolDriver, ToolChatMessage, ToolSession, ToolUsage},
    model::{FieldType, ToolFieldSchema, ToolFormSchema, ToolKind},
};
use serde_json::{json, Value};
use std::process::Command;

const PERMISSION_MODES: &[&str] = &[
    "acceptEdits",
    "bypassPermissions",
    "default",
    "delegate",
    "dontAsk",
    "plan",
];

fn auto_accept_permissions_enabled(config: &Value) -> bool {
    config
        .get("auto_accept_permissions")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn shell_quote(arg: &str) -> String {
    if arg
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '=' | '/'))
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

fn claude_chat_cli_args(config: &Value) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = config.get("model").and_then(Value::as_str) {
        let model = model.trim();
        if !model.is_empty() {
            args.push("--model".to_string());
            args.push(model.to_string());
        }
    }
    if auto_accept_permissions_enabled(config) {
        args.push("--dangerously-skip-permissions".to_string());
    } else if let Some(mode) = config.get("permission_mode").and_then(Value::as_str) {
        let mode = mode.trim();
        if !mode.is_empty() {
            args.push("--permission-mode".to_string());
            args.push(mode.to_string());
        }
    }
    args
}

pub struct ClaudeCodeDriver;

impl CodingToolDriver for ClaudeCodeDriver {
    fn kind(&self) -> ToolKind {
        ToolKind::ClaudeCode
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn schema(&self) -> ToolFormSchema {
        ToolFormSchema {
            title: "Claude Code".to_string(),
            fields: vec![
                ToolFieldSchema {
                    key: "command".to_string(),
                    label: "Command".to_string(),
                    field_type: FieldType::Text,
                    required: true,
                    options: vec![],
                    placeholder: Some("claude".to_string()),
                },
                ToolFieldSchema {
                    key: "model".to_string(),
                    label: "Model".to_string(),
                    field_type: FieldType::Text,
                    required: true,
                    options: vec![],
                    placeholder: Some("claude-sonnet-4".to_string()),
                },
            ],
        }
    }

    fn default_config(&self) -> Value {
        json!({
            "command":"claude",
            "model":"claude-sonnet-4",
            "api_key_env":"ANTHROPIC_API_KEY",
            "prompt_mode":"stdin",
            "auto_accept_permissions": true,
            "permission_mode":"bypassPermissions"
        })
    }

    fn validate(&self, config: &Value) -> anyhow::Result<()> {
        let command = config.get("command").and_then(Value::as_str).unwrap_or("").trim();
        if command.is_empty() {
            anyhow::bail!("command is required");
        }
        let prompt_mode = config
            .get("prompt_mode")
            .and_then(Value::as_str)
            .unwrap_or("stdin");
        if prompt_mode != "stdin" && prompt_mode != "arg" {
            anyhow::bail!("prompt_mode must be stdin or arg");
        }
        if let Some(mode) = config.get("permission_mode").and_then(Value::as_str) {
            let mode = mode.trim();
            if !mode.is_empty() && !PERMISSION_MODES.contains(&mode) {
                anyhow::bail!("permission_mode must be one of: {}", PERMISSION_MODES.join(", "));
            }
        }
        Ok(())
    }

    fn check_installed(&self, config: &Value) -> anyhow::Result<bool> {
        self.validate(config)?;
        let command = config.get("command").and_then(Value::as_str).unwrap_or("claude");
        Ok(Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {} >/dev/null 2>&1", command))
            .status()
            .map(|s| s.success())
            .unwrap_or(false))
    }

    fn install_tool(&self, config: &Value) -> anyhow::Result<()> {
        self.validate(config)?;
        if self.check_installed(config)? {
            return Ok(());
        }

        let status = Command::new("sh")
            .arg("-c")
            .arg("npm install -g @anthropic-ai/claude-code")
            .status()?;
        if !status.success() {
            anyhow::bail!("failed to install Claude Code with npm");
        }
        if !self.check_installed(config)? {
            anyhow::bail!("claude command still not found after installation");
        }
        Ok(())
    }

    fn configure_tool(&self, config: &Value) -> anyhow::Result<Value> {
        self.validate(config)?;
        let mut merged = self.default_config();
        if let (Some(src), Some(dst)) = (config.as_object(), merged.as_object_mut()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
        Ok(merged)
    }

    fn start_tool(&self, config: &Value) -> anyhow::Result<()> {
        self.validate(config)?;
        let command = config.get("command").and_then(Value::as_str).unwrap_or("claude");
        let status = Command::new(command).arg("--version").status();
        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(_) => anyhow::bail!("claude command failed to start"),
            Err(err) => anyhow::bail!("failed to execute claude command: {err}"),
        }
    }

    fn create_session(&self, config: &Value) -> anyhow::Result<ToolSession> {
        self.start_tool(config)?;
        Ok(ToolSession {
            id: format!(
                "claude_session_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis()
            ),
            started_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis(),
        })
    }

    fn chat_subprocess_spec(&self, config: &Value, messages: &[ToolChatMessage]) -> anyhow::Result<ChatSubprocessSpec> {
        self.validate(config)?;
        let command = config.get("command").and_then(Value::as_str).unwrap_or("claude");
        let prompt_mode = config
            .get("prompt_mode")
            .and_then(Value::as_str)
            .unwrap_or("stdin");
        let prompt = crate::tools::driver::join_chat_prompt(messages);
        let cli_args = claude_chat_cli_args(config);
        let cli_prefix = cli_args.iter().map(|arg| shell_quote(arg)).collect::<Vec<_>>().join(" ");

        if prompt_mode == "arg" {
            let mut args = cli_args;
            args.push("-p".to_string());
            args.push(prompt);
            Ok(ChatSubprocessSpec {
                program: command.to_string(),
                args,
                env: vec![],
            })
        } else {
            let command_with_flags = if cli_prefix.is_empty() {
                command.to_string()
            } else {
                format!("{command} {cli_prefix}")
            };
            Ok(ChatSubprocessSpec {
                program: "sh".to_string(),
                args: vec![
                    "-c".to_string(),
                    format!("printf %s \"$PROMPT\" | {command_with_flags} -p -"),
                ],
                env: vec![("PROMPT".to_string(), prompt)],
            })
        }
    }

    fn collect_usage(
        &self,
        config: &Value,
        messages: &[ToolChatMessage],
        completion: &str,
    ) -> anyhow::Result<ToolUsage> {
        let prompt_chars: usize = messages
            .iter()
            .map(|m| m.role.chars().count() + m.content.chars().count())
            .sum();
        let completion_chars = completion.chars().count();
        let prompt_tokens = ((prompt_chars as f64) / 4.0).ceil() as u64;
        let completion_tokens = ((completion_chars as f64) / 4.0).ceil() as u64;
        Ok(ToolUsage {
            model: config
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("claude-sonnet-4")
                .to_string(),
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::driver::{CodingToolDriver, ToolChatMessage};

    fn sample_messages() -> Vec<ToolChatMessage> {
        vec![ToolChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }]
    }

    #[test]
    fn default_chat_spec_skips_permission_prompts() {
        let driver = ClaudeCodeDriver;
        let config = driver.default_config();
        let spec = driver
            .chat_subprocess_spec(&config, &sample_messages())
            .expect("spec");
        let joined = spec.args.join(" ");
        assert!(joined.contains("--dangerously-skip-permissions"));
        assert!(joined.contains("--model"));
    }

    #[test]
    fn arg_mode_passes_auto_accept_flags_before_prompt() {
        let driver = ClaudeCodeDriver;
        let mut config = driver.default_config();
        config["prompt_mode"] = json!("arg");
        let spec = driver
            .chat_subprocess_spec(&config, &sample_messages())
            .expect("spec");
        assert_eq!(spec.program, "claude");
        let skip_idx = spec
            .args
            .iter()
            .position(|a| a == "--dangerously-skip-permissions")
            .expect("skip flag");
        let prompt_idx = spec.args.iter().position(|a| a == "-p").expect("-p flag");
        assert!(skip_idx < prompt_idx);
    }

    #[test]
    fn permission_mode_used_when_auto_accept_disabled() {
        let driver = ClaudeCodeDriver;
        let mut config = driver.default_config();
        config["auto_accept_permissions"] = json!(false);
        config["prompt_mode"] = json!("arg");
        let spec = driver
            .chat_subprocess_spec(&config, &sample_messages())
            .expect("spec");
        assert!(spec.args.contains(&"--permission-mode".to_string()));
        assert!(spec.args.contains(&"bypassPermissions".to_string()));
        assert!(!spec
            .args
            .iter()
            .any(|a| a == "--dangerously-skip-permissions"));
    }
}
