use crate::tools::{
    driver::{
        truncate_for_preview, ChatStreamEvent, ChatSubprocessSpec, CodingToolDriver,
        StreamParseState, ToolChatMessage, ToolSession, ToolUsage, TOOL_PREVIEW_MAX_CHARS,
    },
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

/// Extra flags that turn Claude Code into a JSONL streaming source.
fn claude_stream_cli_args() -> Vec<String> {
    vec![
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--include-partial-messages".to_string(),
    ]
}

fn input_summary_from(value: &Value) -> String {
    let raw = match value {
        Value::String(s) => s.clone(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    };
    truncate_for_preview(&raw, TOOL_PREVIEW_MAX_CHARS)
}

fn tool_result_content_text(content: &Value) -> (String, bool) {
    match content {
        Value::String(s) => (s.clone(), false),
        Value::Array(items) => {
            let mut buf = String::new();
            let mut is_err = false;
            for item in items {
                if let Some(t) = item.get("text").and_then(Value::as_str) {
                    if !buf.is_empty() {
                        buf.push('\n');
                    }
                    buf.push_str(t);
                }
                if item
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    is_err = true;
                }
            }
            (buf, is_err)
        }
        Value::Null => (String::new(), false),
        other => (other.to_string(), false),
    }
}

fn handle_stream_event(state: &mut StreamParseState, line: &Value) -> Vec<ChatStreamEvent> {
    let Some(event) = line.get("event") else {
        return vec![];
    };
    let ty = event.get("type").and_then(Value::as_str).unwrap_or("");
    match ty {
        "content_block_delta" => {
            let delta = event.get("delta");
            let delta_type = delta
                .and_then(|d| d.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("");
            match delta_type {
                "text_delta" => {
                    let text = delta
                        .and_then(|d| d.get("text"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if text.is_empty() {
                        return vec![];
                    }
                    // Mark message as having streamed text so the final assistant block
                    // doesn't re-emit it verbatim.
                    if let Some(message_id) = line.get("parent_tool_use_id").and_then(Value::as_str)
                    {
                        state
                            .messages_with_text_deltas
                            .insert(message_id.to_string());
                    }
                    vec![ChatStreamEvent::AssistantText {
                        text: text.to_string(),
                    }]
                }
                "thinking_delta" => {
                    let text = delta
                        .and_then(|d| d.get("thinking"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if text.is_empty() {
                        return vec![];
                    }
                    vec![ChatStreamEvent::Thinking {
                        text: text.to_string(),
                    }]
                }
                _ => vec![],
            }
        }
        "message_start" => {
            if let Some(id) = event
                .get("message")
                .and_then(|m| m.get("id"))
                .and_then(Value::as_str)
            {
                // Reserve slot; actual marking happens when deltas arrive.
                let _ = id;
            }
            vec![]
        }
        _ => vec![],
    }
}

fn handle_assistant_message(state: &mut StreamParseState, message: &Value) -> Vec<ChatStreamEvent> {
    let mut out = Vec::new();
    let message_id = message
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let had_partial_text = state.messages_with_text_deltas.contains(&message_id);
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return out;
    };
    for block in content {
        let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
        match block_type {
            "text" => {
                if had_partial_text {
                    continue;
                }
                let text = block.get("text").and_then(Value::as_str).unwrap_or("");
                if !text.is_empty() {
                    out.push(ChatStreamEvent::AssistantText {
                        text: text.to_string(),
                    });
                }
            }
            "thinking" => {
                let text = block.get("thinking").and_then(Value::as_str).unwrap_or("");
                if !text.is_empty() {
                    out.push(ChatStreamEvent::Thinking {
                        text: text.to_string(),
                    });
                }
            }
            "tool_use" => {
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let input = block.get("input").cloned().unwrap_or(Value::Null);
                out.push(ChatStreamEvent::ToolUse {
                    id,
                    name,
                    input_summary: input_summary_from(&input),
                });
            }
            _ => {}
        }
    }
    out
}

fn handle_user_message(message: &Value) -> Vec<ChatStreamEvent> {
    let mut out = Vec::new();
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return out;
    };
    for block in content {
        if block.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }
        let id = block
            .get("tool_use_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let raw_content = block.get("content").cloned().unwrap_or(Value::Null);
        let (text, content_is_err) = tool_result_content_text(&raw_content);
        let is_error = block
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || content_is_err;
        out.push(ChatStreamEvent::ToolResult {
            tool_use_id: id,
            output_preview: truncate_for_preview(&text, TOOL_PREVIEW_MAX_CHARS),
            is_error,
        });
    }
    out
}

fn handle_result_event(line: &Value) -> Vec<ChatStreamEvent> {
    let usage = line.get("usage");
    let input_tokens = usage
        .and_then(|u| u.get("input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_creation = usage
        .and_then(|u| u.get("cache_creation_input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = usage
        .and_then(|u| u.get("cache_read_input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|u| u.get("output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let prompt_tokens = input_tokens + cache_creation + cache_read;
    let completion_tokens = output_tokens;
    let total_tokens = prompt_tokens + completion_tokens;
    let summary = line
        .get("result")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let model = line
        .get("modelUsage")
        .and_then(Value::as_object)
        .and_then(|m| m.keys().next().cloned());
    let is_error = line
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || line.get("subtype").and_then(Value::as_str) == Some("error");
    vec![ChatStreamEvent::Result {
        summary,
        model,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        is_error,
    }]
}

fn parse_one_line(state: &mut StreamParseState, line: &str) -> Vec<ChatStreamEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return vec![];
    }
    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => {
            // Not valid JSON — surface as raw text so users still see output.
            return vec![ChatStreamEvent::Raw {
                text: format!("{trimmed}\n"),
            }];
        }
    };
    let ty = value.get("type").and_then(Value::as_str).unwrap_or("");
    match ty {
        "system" => {
            if value.get("subtype").and_then(Value::as_str) == Some("init") {
                let model = value
                    .get("model")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string());
                let session_id = value
                    .get("session_id")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string());
                let tools = value
                    .get("tools")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let cwd = value
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string());
                return vec![ChatStreamEvent::Start {
                    model,
                    session_id,
                    tools,
                    cwd,
                }];
            }
            vec![]
        }
        "stream_event" => handle_stream_event(state, &value),
        "assistant" => value
            .get("message")
            .map(|m| handle_assistant_message(state, m))
            .unwrap_or_default(),
        "user" => value
            .get("message")
            .map(handle_user_message)
            .unwrap_or_default(),
        "result" => handle_result_event(&value),
        _ => vec![],
    }
}

fn parse_stream_chunk_impl(state: &mut StreamParseState, chunk: &str) -> Vec<ChatStreamEvent> {
    if chunk.is_empty() {
        return vec![];
    }
    state.buffer.push_str(chunk);
    let mut events = Vec::new();
    loop {
        let Some(idx) = state.buffer.find('\n') else {
            break;
        };
        let line: String = state.buffer.drain(..=idx).collect();
        events.extend(parse_one_line(state, &line));
    }
    events
}

fn finalize_stream_impl(state: &mut StreamParseState) -> Vec<ChatStreamEvent> {
    let tail = std::mem::take(&mut state.buffer);
    if tail.trim().is_empty() {
        return vec![];
    }
    parse_one_line(state, &tail)
}

fn build_claude_spec(
    config: &Value,
    messages: &[ToolChatMessage],
    extra_args: &[String],
) -> anyhow::Result<ChatSubprocessSpec> {
    ClaudeCodeDriver.validate(config)?;
    let command = config
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("claude");
    let prompt_mode = config
        .get("prompt_mode")
        .and_then(Value::as_str)
        .unwrap_or("stdin");
    let prompt = crate::tools::driver::join_chat_prompt(messages);
    let mut cli_args = claude_chat_cli_args(config);
    cli_args.extend(extra_args.iter().cloned());
    let cli_prefix = cli_args
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");

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
        build_claude_spec(config, messages, &[])
    }

    fn chat_subprocess_spec_stream(
        &self,
        config: &Value,
        messages: &[ToolChatMessage],
    ) -> anyhow::Result<ChatSubprocessSpec> {
        build_claude_spec(config, messages, &claude_stream_cli_args())
    }

    fn parse_stream_chunk(
        &self,
        state: &mut StreamParseState,
        chunk: &str,
    ) -> Vec<ChatStreamEvent> {
        parse_stream_chunk_impl(state, chunk)
    }

    fn finalize_stream(&self, state: &mut StreamParseState) -> Vec<ChatStreamEvent> {
        finalize_stream_impl(state)
    }

    fn assistant_text_from_events(&self, events: &[ChatStreamEvent]) -> Option<String> {
        let mut delta_buf = String::new();
        let mut result_summary: Option<String> = None;
        for ev in events {
            match ev {
                ChatStreamEvent::AssistantText { text } => delta_buf.push_str(text),
                ChatStreamEvent::Result { summary: Some(s), .. } => {
                    result_summary = Some(s.clone());
                }
                _ => {}
            }
        }
        if !delta_buf.trim().is_empty() {
            Some(delta_buf)
        } else if result_summary.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false) {
            result_summary
        } else {
            None
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

    #[test]
    fn stream_spec_enables_stream_json_with_partial_messages_in_arg_mode() {
        let driver = ClaudeCodeDriver;
        let mut config = driver.default_config();
        config["prompt_mode"] = json!("arg");
        let spec = driver
            .chat_subprocess_spec_stream(&config, &sample_messages())
            .expect("stream spec");
        assert_eq!(spec.program, "claude");
        let joined = spec.args.join(" ");
        assert!(joined.contains("--output-format stream-json"));
        assert!(joined.contains("--verbose"));
        assert!(joined.contains("--include-partial-messages"));
        let p_idx = spec.args.iter().position(|a| a == "-p").expect("-p flag");
        let stream_idx = spec
            .args
            .iter()
            .position(|a| a == "--output-format")
            .expect("stream flag");
        assert!(stream_idx < p_idx, "stream flags must precede prompt");
    }

    #[test]
    fn stream_spec_in_stdin_mode_pipes_through_shell_with_stream_flags() {
        let driver = ClaudeCodeDriver;
        let config = driver.default_config();
        assert_eq!(
            config.get("prompt_mode").and_then(Value::as_str),
            Some("stdin")
        );
        let spec = driver
            .chat_subprocess_spec_stream(&config, &sample_messages())
            .expect("stream spec");
        assert_eq!(spec.program, "sh");
        assert_eq!(spec.args.first().map(String::as_str), Some("-c"));
        let cmd = &spec.args[1];
        assert!(cmd.contains("--output-format stream-json"));
        assert!(cmd.contains("--include-partial-messages"));
        assert!(cmd.contains("| claude"));
        assert!(cmd.contains("-p -"));
    }

    #[test]
    fn parser_emits_start_event_from_system_init_line() {
        let driver = ClaudeCodeDriver;
        let mut state = StreamParseState::default();
        let line = "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"abc\",\"model\":\"claude-sonnet-4\",\"tools\":[\"Bash\",\"Edit\"],\"cwd\":\"/work\"}\n";
        let events = driver.parse_stream_chunk(&mut state, line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatStreamEvent::Start {
                model,
                session_id,
                tools,
                cwd,
            } => {
                assert_eq!(model.as_deref(), Some("claude-sonnet-4"));
                assert_eq!(session_id.as_deref(), Some("abc"));
                assert_eq!(tools, &vec!["Bash".to_string(), "Edit".to_string()]);
                assert_eq!(cwd.as_deref(), Some("/work"));
            }
            other => panic!("expected Start, got {other:?}"),
        }
    }

    #[test]
    fn parser_emits_text_chunks_from_partial_content_block_deltas() {
        let driver = ClaudeCodeDriver;
        let mut state = StreamParseState::default();
        let chunk = concat!(
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}}\n",
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}}\n",
        );
        let events = driver.parse_stream_chunk(&mut state, chunk);
        let texts: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                ChatStreamEvent::AssistantText { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["Hel", "lo"]);
    }

    #[test]
    fn parser_buffers_partial_lines_until_newline() {
        let driver = ClaudeCodeDriver;
        let mut state = StreamParseState::default();
        let first = driver.parse_stream_chunk(
            &mut state,
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi",
        );
        assert!(first.is_empty(), "no events until newline arrives");
        let second = driver.parse_stream_chunk(&mut state, "\"}}}\n");
        assert_eq!(
            second,
            vec![ChatStreamEvent::AssistantText {
                text: "hi".to_string()
            }]
        );
    }

    #[test]
    fn parser_emits_tool_use_event_from_assistant_message() {
        let driver = ClaudeCodeDriver;
        let mut state = StreamParseState::default();
        let line = "{\"type\":\"assistant\",\"message\":{\"id\":\"msg_1\",\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"id\":\"tu_1\",\"name\":\"Bash\",\"input\":{\"command\":\"ls\"}}]}}\n";
        let events = driver.parse_stream_chunk(&mut state, line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatStreamEvent::ToolUse {
                id,
                name,
                input_summary,
            } => {
                assert_eq!(id, "tu_1");
                assert_eq!(name, "Bash");
                assert!(input_summary.contains("\"command\""));
                assert!(input_summary.contains("ls"));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn parser_emits_tool_result_event_from_user_message() {
        let driver = ClaudeCodeDriver;
        let mut state = StreamParseState::default();
        let line = "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"tu_1\",\"content\":\"hello world\",\"is_error\":false}]}}\n";
        let events = driver.parse_stream_chunk(&mut state, line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatStreamEvent::ToolResult {
                tool_use_id,
                output_preview,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tu_1");
                assert_eq!(output_preview, "hello world");
                assert!(!*is_error);
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn parser_emits_result_event_with_usage_totals() {
        let driver = ClaudeCodeDriver;
        let mut state = StreamParseState::default();
        let line = "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"all done\",\"usage\":{\"input_tokens\":3,\"cache_creation_input_tokens\":1,\"cache_read_input_tokens\":2,\"output_tokens\":4}}\n";
        let events = driver.parse_stream_chunk(&mut state, line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatStreamEvent::Result {
                summary,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                is_error,
                ..
            } => {
                assert_eq!(summary.as_deref(), Some("all done"));
                assert_eq!(*prompt_tokens, 6);
                assert_eq!(*completion_tokens, 4);
                assert_eq!(*total_tokens, 10);
                assert!(!*is_error);
            }
            other => panic!("expected Result, got {other:?}"),
        }
    }

    #[test]
    fn parser_falls_back_to_raw_text_for_invalid_json_lines() {
        let driver = ClaudeCodeDriver;
        let mut state = StreamParseState::default();
        let events = driver.parse_stream_chunk(&mut state, "not json at all\n");
        assert_eq!(
            events,
            vec![ChatStreamEvent::Raw {
                text: "not json at all\n".to_string()
            }]
        );
    }

    #[test]
    fn finalize_emits_remaining_buffered_line() {
        let driver = ClaudeCodeDriver;
        let mut state = StreamParseState::default();
        // No trailing newline; should not parse during chunk.
        let _ = driver.parse_stream_chunk(
            &mut state,
            "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"usage\":{\"input_tokens\":0,\"output_tokens\":0}}",
        );
        let final_events = driver.finalize_stream(&mut state);
        assert!(matches!(
            final_events.first(),
            Some(ChatStreamEvent::Result { .. })
        ));
    }

    #[test]
    fn assistant_text_from_events_concatenates_deltas() {
        let driver = ClaudeCodeDriver;
        let events = vec![
            ChatStreamEvent::AssistantText {
                text: "Hello".into(),
            },
            ChatStreamEvent::ToolUse {
                id: "t".into(),
                name: "Bash".into(),
                input_summary: "{}".into(),
            },
            ChatStreamEvent::AssistantText {
                text: " world".into(),
            },
        ];
        assert_eq!(
            driver.assistant_text_from_events(&events),
            Some("Hello world".to_string())
        );
    }
}
