use axum::http::HeaderMap;
use crate::tools::driver::ToolChatMessage;
use std::{fs, path::Path};

const LOCALE_FILE: &str = "settings/locale.json";

pub fn normalize_lang(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.starts_with("zh") {
        "zh"
    } else if trimmed.starts_with("ja") {
        "ja"
    } else {
        "en"
    }
}

pub fn resolve_lang(headers: &HeaderMap) -> &'static str {
    crate::i18n::resolve_lang(headers)
}

pub fn sync_lang_from_headers(headers: &HeaderMap, workspace: &Path) {
    let lang = resolve_lang(headers);
    let _ = save_workspace_lang(workspace, lang);
}

pub fn save_workspace_lang(workspace: &Path, lang: &str) -> anyhow::Result<()> {
    let path = workspace.join(LOCALE_FILE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::json!({ "lang": normalize_lang(lang) });
    fs::write(path, serde_json::to_string_pretty(&payload)?)?;
    Ok(())
}

pub fn load_workspace_lang(workspace: &Path) -> Option<&'static str> {
    let path = workspace.join(LOCALE_FILE);
    let raw = fs::read_to_string(path).ok()?;
    let payload: serde_json::Value = serde_json::from_str(&raw).ok()?;
    payload
        .get("lang")
        .and_then(|value| value.as_str())
        .map(normalize_lang)
}

pub fn resolve_lang_for_workspace(workspace: &Path) -> &'static str {
    load_workspace_lang(workspace).unwrap_or("en")
}

pub fn agent_language_directive(lang: &str) -> String {
    crate::i18n::agent_language_directive(lang)
}

pub fn ensure_language_system_message(messages: Vec<ToolChatMessage>, lang: &str) -> Vec<ToolChatMessage> {
    let directive = agent_language_directive(lang);
    if directive.trim().is_empty() {
        return messages;
    }
    if let Some(first) = messages.first() {
        if first.role == "system" {
            let mut out = messages;
            if let Some(system) = out.first_mut() {
                if !system.content.contains(&directive) {
                    system.content = format!("{}\n\n{directive}", system.content);
                }
            }
            return out;
        }
    }
    let mut out = vec![ToolChatMessage {
        role: "system".into(),
        content: directive,
    }];
    out.extend(messages);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn sync_and_load_workspace_lang() {
        let workspace = std::env::temp_dir().join(format!(
            "kaisha-locale-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut headers = HeaderMap::new();
        headers.insert("x-lang", HeaderValue::from_static("zh"));
        sync_lang_from_headers(&headers, &workspace);
        assert_eq!(load_workspace_lang(&workspace), Some("zh"));
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn ensure_language_system_message_appends_to_existing_system_prompt() {
        let messages = vec![
            ToolChatMessage {
                role: "system".into(),
                content: "You are a planner.".into(),
            },
            ToolChatMessage {
                role: "user".into(),
                content: "Plan work.".into(),
            },
        ];
        let localized = ensure_language_system_message(messages, "zh");
        assert_eq!(localized.len(), 2);
        assert!(localized[0].content.contains("You are a planner."));
        assert!(localized[0].content.contains("简体中文"));
    }

    #[test]
    fn ensure_language_system_message_inserts_system_prompt_when_missing() {
        let messages = vec![ToolChatMessage {
            role: "user".into(),
            content: "Run task.".into(),
        }];
        let localized = ensure_language_system_message(messages, "ja");
        assert_eq!(localized.len(), 2);
        assert_eq!(localized[0].role, "system");
        assert!(localized[0].content.contains("日本語"));
    }
}
