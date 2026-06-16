use crate::tools::{
    driver::{
        truncate_for_preview, ChatStreamEvent, ChatSubprocessSpec, CodingToolDriver,
        StreamParseState, ToolChatMessage, ToolSession, ToolUsage, TOOL_PREVIEW_MAX_CHARS,
    },
    model::{FieldType, ToolFieldSchema, ToolFormSchema, ToolKind},
};
use serde_json::{json, Value};
use std::process::Command;

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

fn normalize_cursor_model(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "auto".to_string();
    }
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "auto" => "auto".to_string(),
        "cursor-agent" | "cursor_agent" => "auto".to_string(),
        _ => trimmed.to_string(),
    }
}

fn cursor_chat_cli_args(config: &Value) -> Vec<String> {
    let mut args = vec!["-p".to_string()];
    if auto_accept_permissions_enabled(config) {
        args.push("--trust".to_string());
        args.push("--force".to_string());
        args.push("--approve-mcps".to_string());
    }
    if let Some(model) = config.get("model").and_then(Value::as_str) {
        let model = normalize_cursor_model(model);
        if !model.is_empty() {
            args.push("--model".to_string());
            args.push(model);
        }
    }
    args
}

fn cursor_api_key_env(config: &Value) -> Vec<(String, String)> {
    let env_name = config
        .get("api_key_env")
        .and_then(Value::as_str)
        .unwrap_or("CURSOR_API_KEY")
        .trim();
    if env_name.is_empty() {
        return vec![];
    }
    match std::env::var(env_name) {
        Ok(key) if !key.trim().is_empty() => vec![("CURSOR_API_KEY".to_string(), key)],
        _ => vec![],
    }
}

fn cursor_stream_cli_args() -> Vec<String> {
    vec![
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--stream-partial-output".to_string(),
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

fn handle_assistant_message(state: &mut StreamParseState, line: &Value) -> Vec<ChatStreamEvent> {
    let session_id = line
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let has_partial_marker = line.get("timestamp_ms").is_some();
    if has_partial_marker && !session_id.is_empty() {
        state.messages_with_text_deltas.insert(session_id.clone());
    }
    if !has_partial_marker
        && !session_id.is_empty()
        && state.messages_with_text_deltas.contains(&session_id)
    {
        return vec![];
    }

    let mut out = Vec::new();
    let Some(message) = line.get("message") else {
        return out;
    };
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return out;
    };
    for block in content {
        let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
        match block_type {
            "text" => {
                let text = block.get("text").and_then(Value::as_str).unwrap_or("");
                if !text.is_empty() {
                    out.push(ChatStreamEvent::AssistantText {
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
        .and_then(|u| u.get("inputTokens").or_else(|| u.get("input_tokens")))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = usage
        .and_then(|u| u.get("cacheReadTokens").or_else(|| u.get("cache_read_tokens")))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_write = usage
        .and_then(|u| u.get("cacheWriteTokens").or_else(|| u.get("cache_write_tokens")))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|u| u.get("outputTokens").or_else(|| u.get("output_tokens")))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let prompt_tokens = input_tokens + cache_read + cache_write;
    let completion_tokens = output_tokens;
    let total_tokens = prompt_tokens + completion_tokens;
    let summary = line
        .get("result")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let model = line
        .get("model")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
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
                let cwd = value
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string());
                return vec![ChatStreamEvent::Start {
                    model,
                    session_id,
                    tools: vec![],
                    cwd,
                }];
            }
            vec![]
        }
        "assistant" => handle_assistant_message(state, &value),
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

fn build_cursor_spec(
    config: &Value,
    messages: &[ToolChatMessage],
    extra_args: &[String],
) -> anyhow::Result<ChatSubprocessSpec> {
    CursorCliDriver.validate(config)?;
    let command = config
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("cursor");
    let prompt_mode = config
        .get("prompt_mode")
        .and_then(Value::as_str)
        .unwrap_or("arg");
    let prompt = crate::tools::driver::join_chat_prompt(messages);
    let mut cli_args = cursor_chat_cli_args(config);
    cli_args.extend(extra_args.iter().cloned());
    let cli_prefix = cli_args
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");

    if prompt_mode == "stdin" {
        let command_with_flags = if cli_prefix.is_empty() {
            format!("{command} agent")
        } else {
            format!("{command} agent {cli_prefix}")
        };
        let mut env = cursor_api_key_env(config);
        env.push(("PROMPT".to_string(), prompt));
        Ok(ChatSubprocessSpec {
            program: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                format!("printf %s \"$PROMPT\" | {command_with_flags} -"),
            ],
            env,
        })
    } else {
        let mut args = vec!["agent".to_string()];
        args.extend(cli_args);
        args.push(prompt);
        Ok(ChatSubprocessSpec {
            program: command.to_string(),
            args,
            env: cursor_api_key_env(config),
        })
    }
}

pub struct CursorCliDriver;

const CURSOR_MODEL_OPTIONS: &[&str] = &[
    "auto",
    "composer-2.5",
    "composer-2.5-fast",
    "grok-build-0.1",
    "grok-4.3",
    "kimi-k2.5",
];

impl CodingToolDriver for CursorCliDriver {
    fn kind(&self) -> ToolKind {
        ToolKind::CursorCli
    }
    fn display_name(&self) -> &'static str {
        "Cursor CLI"
    }
    fn schema(&self) -> ToolFormSchema {
        ToolFormSchema {
            title: "Cursor CLI".to_string(),
            fields: vec![
                ToolFieldSchema {
                    key: "command".to_string(),
                    label: "Command".to_string(),
                    field_type: FieldType::Text,
                    required: true,
                    options: vec![],
                    placeholder: Some("cursor".to_string()),
                },
                ToolFieldSchema {
                    key: "model".to_string(),
                    label: "Model".to_string(),
                    field_type: FieldType::Combobox,
                    required: false,
                    options: CURSOR_MODEL_OPTIONS
                        .iter()
                        .map(|item| (*item).to_string())
                        .collect(),
                    placeholder: Some("auto".to_string()),
                },
                ToolFieldSchema {
                    key: "auto_accept_permissions".to_string(),
                    label: "Auto accept permissions".to_string(),
                    field_type: FieldType::Boolean,
                    required: false,
                    options: vec![],
                    placeholder: None,
                },
            ],
        }
    }
    fn default_config(&self) -> Value {
        json!({
            "command":"cursor",
            "model":"auto",
            "api_key_env":"CURSOR_API_KEY",
            "prompt_mode":"arg",
            "auto_accept_permissions": true
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
            .unwrap_or("arg");
        if prompt_mode != "stdin" && prompt_mode != "arg" {
            anyhow::bail!("prompt_mode must be stdin or arg");
        }
        Ok(())
    }

    fn check_installed(&self, config: &Value) -> anyhow::Result<bool> {
        self.validate(config)?;
        let command = config.get("command").and_then(Value::as_str).unwrap_or("cursor");
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
        anyhow::bail!("cursor cli installation is platform-specific; please install manually")
    }

    fn configure_tool(&self, config: &Value) -> anyhow::Result<Value> {
        self.validate(config)?;
        let mut merged = self.default_config();
        if let (Some(src), Some(dst)) = (config.as_object(), merged.as_object_mut()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
        if let Some(model) = merged.get("model").and_then(Value::as_str) {
            merged["model"] = json!(normalize_cursor_model(model));
        }
        if let Some(obj) = merged.as_object_mut() {
            obj.remove("profile");
        }
        Ok(merged)
    }

    fn start_tool(&self, config: &Value) -> anyhow::Result<()> {
        self.validate(config)?;
        let command = config.get("command").and_then(Value::as_str).unwrap_or("cursor");
        let status = Command::new(command).arg("agent").arg("--version").status();
        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(_) => anyhow::bail!("cursor agent command failed to start"),
            Err(err) => anyhow::bail!("failed to execute cursor command: {err}"),
        }
    }

    fn create_session(&self, config: &Value) -> anyhow::Result<ToolSession> {
        self.start_tool(config)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();
        Ok(ToolSession {
            id: format!("cursor_session_{now}"),
            started_at_ms: now,
        })
    }

    fn chat_subprocess_spec(&self, config: &Value, messages: &[ToolChatMessage]) -> anyhow::Result<ChatSubprocessSpec> {
        build_cursor_spec(config, messages, &[])
    }

    fn chat_subprocess_spec_stream(
        &self,
        config: &Value,
        messages: &[ToolChatMessage],
    ) -> anyhow::Result<ChatSubprocessSpec> {
        build_cursor_spec(config, messages, &cursor_stream_cli_args())
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
                .unwrap_or("auto")
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
    fn model_field_uses_combobox_with_known_options() {
        let driver = CursorCliDriver;
        let model_field = driver
            .schema()
            .fields
            .into_iter()
            .find(|field| field.key == "model")
            .expect("model field");
        assert_eq!(model_field.field_type, FieldType::Combobox);
        assert!(model_field.options.contains(&"auto".to_string()));
        assert!(model_field.options.contains(&"composer-2.5".to_string()));
    }

    #[test]
    fn configure_tool_strips_legacy_profile_and_normalizes_model() {
        let driver = CursorCliDriver;
        let config = json!({
            "command": "cursor",
            "model": "cursor-agent",
            "profile": "default",
            "prompt_mode": "arg"
        });
        let merged = driver.configure_tool(&config).expect("configure");
        assert_eq!(merged.get("model").and_then(Value::as_str), Some("auto"));
        assert!(merged.get("profile").is_none());
    }

    #[test]
    fn legacy_cursor_agent_model_is_normalized_to_auto() {
        let driver = CursorCliDriver;
        let mut config = driver.default_config();
        config["model"] = json!("cursor-agent");
        let spec = driver
            .chat_subprocess_spec(&config, &sample_messages())
            .expect("spec");
        let model_idx = spec
            .args
            .iter()
            .position(|a| a == "--model")
            .expect("--model flag");
        assert_eq!(spec.args.get(model_idx + 1).map(String::as_str), Some("auto"));
    }

    #[test]
    fn default_chat_spec_uses_print_mode_with_trust_and_force() {
        let driver = CursorCliDriver;
        let config = driver.default_config();
        let spec = driver
            .chat_subprocess_spec(&config, &sample_messages())
            .expect("spec");
        assert_eq!(spec.program, "cursor");
        assert!(spec.args.contains(&"agent".to_string()));
        assert!(spec.args.contains(&"-p".to_string()));
        assert!(spec.args.contains(&"--trust".to_string()));
        assert!(spec.args.contains(&"--force".to_string()));
        assert!(spec.args.contains(&"--approve-mcps".to_string()));
        assert!(!spec.args.iter().any(|a| a == "--profile"));
    }

    #[test]
    fn auto_accept_disabled_omits_headless_permission_flags() {
        let driver = CursorCliDriver;
        let mut config = driver.default_config();
        config["auto_accept_permissions"] = json!(false);
        let spec = driver
            .chat_subprocess_spec(&config, &sample_messages())
            .expect("spec");
        assert!(spec.args.contains(&"-p".to_string()));
        assert!(!spec.args.iter().any(|a| a == "--trust"));
        assert!(!spec.args.iter().any(|a| a == "--force"));
        assert!(!spec.args.iter().any(|a| a == "--approve-mcps"));
    }

    #[test]
    fn stdin_mode_pipes_prompt_to_agent_dash() {
        let driver = CursorCliDriver;
        let mut config = driver.default_config();
        config["prompt_mode"] = json!("stdin");
        let spec = driver
            .chat_subprocess_spec_stream(&config, &sample_messages())
            .expect("spec");
        assert_eq!(spec.program, "sh");
        let shell_cmd = spec.args.get(1).expect("shell command");
        assert!(shell_cmd.contains("| cursor agent"));
        assert!(shell_cmd.ends_with(" -"));
        assert!(shell_cmd.contains("--output-format stream-json"));
        assert!(shell_cmd.contains("--stream-partial-output"));
        assert!(spec.env.iter().any(|(k, _)| k == "PROMPT"));
    }

    #[test]
    fn stream_spec_enables_stream_json_with_partial_output() {
        let driver = CursorCliDriver;
        let config = driver.default_config();
        let spec = driver
            .chat_subprocess_spec_stream(&config, &sample_messages())
            .expect("stream spec");
        let joined = spec.args.join(" ");
        assert!(joined.contains("--output-format stream-json"));
        assert!(joined.contains("--stream-partial-output"));
        let print_idx = spec.args.iter().position(|a| a == "-p").expect("-p");
        let stream_idx = spec
            .args
            .iter()
            .position(|a| a == "--output-format")
            .expect("stream flag");
        assert!(print_idx < stream_idx);
    }

    #[test]
    fn parser_emits_partial_assistant_text_and_skips_consolidated_message() {
        let driver = CursorCliDriver;
        let mut state = StreamParseState::default();
        let chunk = concat!(
            "{\"type\":\"assistant\",\"session_id\":\"s1\",\"timestamp_ms\":1,\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Hel\"}]}}\n",
            "{\"type\":\"assistant\",\"session_id\":\"s1\",\"timestamp_ms\":2,\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"lo\"}]}}\n",
            "{\"type\":\"assistant\",\"session_id\":\"s1\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Hello\"}]}}\n",
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
    fn parser_emits_result_event_with_camel_case_usage() {
        let driver = CursorCliDriver;
        let mut state = StreamParseState::default();
        let line = "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"done\",\"usage\":{\"inputTokens\":3,\"cacheReadTokens\":2,\"outputTokens\":4}}\n";
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
                assert_eq!(summary.as_deref(), Some("done"));
                assert_eq!(*prompt_tokens, 5);
                assert_eq!(*completion_tokens, 4);
                assert_eq!(*total_tokens, 9);
                assert!(!*is_error);
            }
            other => panic!("expected Result, got {other:?}"),
        }
    }
}
