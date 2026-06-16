use crate::tools::driver::{
    ChatStreamEvent, ChatSubprocessSpec, CodingToolDriver, StreamParseState,
};
use anyhow::Context;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

/// Runs the chat subprocess and forwards driver-parsed `ChatStreamEvent`s.
///
/// Returns the merged stdout+stderr output, the exit code, and the full ordered list
/// of emitted events (so callers can reconstruct the final assistant text and tool log).
pub async fn stream_chat_events(
    driver: Arc<dyn CodingToolDriver>,
    spec: &ChatSubprocessSpec,
    cwd: &Path,
    event_tx: tokio::sync::mpsc::Sender<ChatStreamEvent>,
) -> anyhow::Result<(String, i32, Vec<ChatStreamEvent>)> {
    let mut cmd = build_command(spec);
    cmd.current_dir(cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().context("spawn chat subprocess")?;

    let stdout = child.stdout.take().context("stdout pipe")?;
    let stderr = child.stderr.take().context("stderr pipe")?;

    let all_events: Arc<Mutex<Vec<ChatStreamEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_acc: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let mut sender_open = true;

    let stderr_events = all_events.clone();
    let stderr_buf = stderr_acc.clone();
    let stderr_tx = event_tx.clone();
    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = vec![0u8; 4096];
        loop {
            let n = match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
            if chunk.is_empty() {
                continue;
            }
            stderr_buf.lock().await.push_str(&chunk);
            let ev = ChatStreamEvent::Raw { text: chunk };
            stderr_events.lock().await.push(ev.clone());
            let _ = stderr_tx.send(ev).await;
        }
    });

    let mut reader = BufReader::new(stdout);
    let mut stdout_acc = String::new();
    let mut buf = vec![0u8; 8192];
    let mut state = StreamParseState::default();

    loop {
        let n = reader.read(&mut buf).await.context("read stdout")?;
        if n == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
        stdout_acc.push_str(&chunk);
        let events = driver.parse_stream_chunk(&mut state, &chunk);
        for ev in events {
            all_events.lock().await.push(ev.clone());
            if sender_open && event_tx.send(ev).await.is_err() {
                sender_open = false;
            }
        }
    }

    for ev in driver.finalize_stream(&mut state) {
        all_events.lock().await.push(ev.clone());
        if sender_open && event_tx.send(ev).await.is_err() {
            sender_open = false;
        }
    }

    let _ = stderr_handle.await;
    let status = child.wait().await.context("wait subprocess")?;
    let code = status.code().unwrap_or(1);
    let stderr_text = stderr_acc.lock().await.clone();
    let merged = crate::tools::driver::merge_shell_output(&stdout_acc, &stderr_text);
    let events = Arc::try_unwrap(all_events)
        .map(|m| m.into_inner())
        .unwrap_or_default();
    Ok((merged, code, events))
}

fn build_command(spec: &ChatSubprocessSpec) -> Command {
    let mut cmd = Command::new(&spec.program);
    for a in &spec.args {
        cmd.arg(a);
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::driver::ToolChatMessage;
    use crate::tools::drivers::cursor_cli::CursorCliDriver;

    #[tokio::test]
    async fn stderr_is_forwarded_while_stdout_is_still_open() {
        let driver = Arc::new(CursorCliDriver) as Arc<dyn CodingToolDriver>;
        let spec = ChatSubprocessSpec {
            program: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "echo 'early err' >&2; sleep 0.05; printf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"ok\"}'".to_string(),
            ],
            env: vec![],
        };
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(16);
        let handle = tokio::spawn(async move {
            stream_chat_events(driver, &spec, Path::new("."), event_tx).await
        });

        let first = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        )
        .await
        .expect("timed out waiting for first stream event")
        .expect("stream closed before first event");
        match first {
            ChatStreamEvent::Raw { text } => assert!(text.contains("early err")),
            other => panic!("expected stderr Raw event first, got {other:?}"),
        }

        let result = handle.await.expect("join stream task").expect("stream run");
        assert_eq!(result.1, 0);
        assert!(result.0.contains("early err"));
    }

    #[test]
    fn build_command_preserves_spec() {
        let spec = ChatSubprocessSpec {
            program: "echo".to_string(),
            args: vec!["hello".to_string()],
            env: vec![("A".to_string(), "1".to_string())],
        };
        let cmd = build_command(&spec);
        assert_eq!(cmd.as_std().get_program(), "echo");
        let _messages = [ToolChatMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        }];
    }
}
