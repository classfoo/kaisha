use crate::{
    requirement::{
        ensure_requirements_root, list_requirement_summaries, load_requirement_detail, phase_in_progress,
        RequirementSummary, REQUIREMENT_FILE,
    },
    tools::driver::ToolChatMessage,
};
use std::path::Path;

const MAX_PRIOR_MESSAGES: usize = 8;
const CONTENT_EXCERPT_CHARS: usize = 480;

/// Working directory for the coding agent during requirement triage.
pub fn requirement_agent_workdir(workspace: &Path) -> anyhow::Result<std::path::PathBuf> {
    ensure_requirements_root(workspace)
}

pub fn format_requirement_catalog(workspace: &Path, items: &[RequirementSummary]) -> anyhow::Result<String> {
    if items.is_empty() {
        return Ok("(no requirements yet)\n".to_string());
    }
    let mut out = String::new();
    for item in items {
        let phase = item.phase.as_str();
        let progress = if phase_in_progress(&item.phase) {
            "in_progress"
        } else {
            "released"
        };
        out.push_str(&format!(
            "- id: {}\n  title: {}\n  phase: {} ({})\n  file: {}/{}\n  updated_at_ms: {}\n",
            item.id, item.title, phase, progress, item.id, REQUIREMENT_FILE, item.updated_at_ms
        ));
        if phase_in_progress(&item.phase) {
            if let Ok(detail) = load_requirement_detail(workspace, &item.id) {
                let excerpt = excerpt_text(&detail.content, CONTENT_EXCERPT_CHARS);
                if !excerpt.is_empty() {
                    out.push_str("  content_excerpt: |\n");
                    for line in excerpt.lines() {
                        out.push_str(&format!("    {line}\n"));
                    }
                }
            }
        }
        out.push('\n');
    }
    Ok(out)
}

fn excerpt_text(content: &str, max_chars: usize) -> String {
    let trimmed = content.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    trimmed.chars().take(max_chars).collect::<String>() + "…"
}

fn system_instructions(catalog: &str) -> String {
    format!(
        r#"You are a requirement management agent. Your ONLY job on this turn is to analyze the user's latest input and organize it into the requirement repository.

## Working directory
Your current working directory IS the requirements root. All paths are relative to this directory (e.g. `my-feature/{req_file}`).

## Requirement storage layout
- Each requirement lives in its own directory: `<requirement-id>/`
- Metadata + body live in `<requirement-id>/{req_file}`
- File format: YAML frontmatter between `---` lines, then Markdown body.

Example:
---
id: user-auth
title: User authentication
phase: collection
created_at_ms: 1710000000000
updated_at_ms: 1710000000000
---

# Goals
...

## Phases (use exactly these values in frontmatter)
- collection — gathering information
- review — under review
- confirm — confirmed scope
- development — implementation
- testing — QA / verification
- release — shipped / closed

## In-progress requirements
Requirements in phases other than `release` are still active. Prefer updating an existing in-progress requirement when the user input clearly extends or refines it. Create a new requirement directory when the input is a distinct new topic.

## Your tasks
1. Read the catalog and any existing files you need.
2. Create or update `{req_file}` files using valid frontmatter (id must match directory name, update `updated_at_ms` on edits, preserve `created_at_ms` on updates).
3. You may create subdirectories under a requirement for process artifacts (notes, designs) when helpful.
4. If the user explicitly asks to **start requirement review** for a requirement id, ensure that requirement's phase is `review` in its frontmatter. Tell them to use chat phrase "进入需求评审" with the requirement id, or use the Review button in the UI (the server runs the multi-role review pipeline).
5. Reply in plain language summarizing what you created or changed (ids, titles, phases). Do not only describe intent — perform the file operations.

## Current catalog
{catalog}
"#,
        req_file = REQUIREMENT_FILE,
        catalog = catalog
    )
}

/// Builds the message list for the coding agent requirement triage turn.
pub fn build_requirement_agent_messages(
    workspace: &Path,
    user_input: &str,
    prior_messages: &[(String, String)],
) -> anyhow::Result<Vec<ToolChatMessage>> {
    let catalog_items = list_requirement_summaries(workspace)?;
    let catalog = format_requirement_catalog(workspace, &catalog_items)?;
    let mut messages = vec![ToolChatMessage {
        role: "system".to_string(),
        content: system_instructions(&catalog),
    }];
    for (role, content) in prior_messages {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            continue;
        }
        messages.push(ToolChatMessage {
            role: role.clone(),
            content: trimmed.to_string(),
        });
    }
    messages.push(ToolChatMessage {
        role: "user".to_string(),
        content: format!(
            "## Latest user input (analyze and apply to requirements now)\n\n{user_input}\n\n\
             Use file tools to create or update requirements in this directory, then summarize what you did."
        ),
    });
    Ok(messages)
}

/// Extracts recent conversation turns as (role, content) pairs before the latest user message.
pub fn prior_conversation_context(
    messages: &[(String, String, u64)],
) -> Vec<(String, String)> {
    let end = messages.len().saturating_sub(1);
    let start = end.saturating_sub(MAX_PRIOR_MESSAGES);
    messages[start..end]
        .iter()
        .map(|(role, content, _)| (role.clone(), content.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requirement::{format_requirement_md, RequirementMeta, RequirementPhase};
    use std::fs;

    fn sample_meta(id: &str, phase: RequirementPhase) -> RequirementMeta {
        RequirementMeta {
            id: id.to_string(),
            title: format!("Title {id}"),
            phase,
            created_at_ms: 1,
            updated_at_ms: 2,
        }
    }

    #[test]
    fn catalog_marks_in_progress_and_includes_excerpt() {
        let workspace = std::env::temp_dir().join(format!(
            "kaisha-req-agent-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&workspace);
        let root = ensure_requirements_root(&workspace).unwrap();
        let dir = root.join("feat-a");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(REQUIREMENT_FILE),
            format_requirement_md(
                &sample_meta("feat-a", RequirementPhase::Collection),
                "## Acceptance\n\nUser can log in.",
            ),
        )
        .unwrap();
        let items = list_requirement_summaries(&workspace).unwrap();
        let catalog = format_requirement_catalog(&workspace, &items).unwrap();
        assert!(catalog.contains("feat-a"));
        assert!(catalog.contains("in_progress"));
        assert!(catalog.contains("User can log in"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn build_messages_starts_with_system_and_ends_with_user() {
        let workspace = std::env::temp_dir().join(format!(
            "kaisha-req-msg-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        ensure_requirements_root(&workspace).unwrap();
        let msgs = build_requirement_agent_messages(&workspace, "Add SSO support", &[]).unwrap();
        assert_eq!(msgs.first().map(|m| m.role.as_str()), Some("system"));
        assert_eq!(msgs.last().map(|m| m.role.as_str()), Some("user"));
        assert!(msgs.last().unwrap().content.contains("Add SSO support"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn prior_context_skips_latest_message() {
        let rows = vec![
            ("user".into(), "a".into(), 1),
            ("assistant".into(), "b".into(), 2),
            ("user".into(), "latest".into(), 3),
        ];
        let prior = prior_conversation_context(&rows);
        assert_eq!(prior.len(), 2);
        assert!(!prior.iter().any(|(_, c)| c == "latest"));
    }
}
