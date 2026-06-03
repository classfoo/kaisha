use crate::tools::driver::{
    ChatStreamEvent, ChatSubprocessSpec, CodingToolDriver, StreamParseState,
};
use anyhow::Context;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;

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

    let mut reader = BufReader::new(stdout);
    let mut stdout_acc = String::new();
    let mut buf = vec![0u8; 8192];
    let mut state = StreamParseState::default();
    let mut all_events: Vec<ChatStreamEvent> = Vec::new();
    let mut sender_open = true;

    loop {
        let n = reader.read(&mut buf).await.context("read stdout")?;
        if n == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
        stdout_acc.push_str(&chunk);
        let events = driver.parse_stream_chunk(&mut state, &chunk);
        for ev in events {
            all_events.push(ev.clone());
            if sender_open && event_tx.send(ev).await.is_err() {
                sender_open = false;
            }
        }
    }

    for ev in driver.finalize_stream(&mut state) {
        all_events.push(ev.clone());
        if sender_open && event_tx.send(ev).await.is_err() {
            sender_open = false;
        }
    }

    let mut stderr_reader = BufReader::new(stderr);
    let mut stderr_acc = String::new();
    stderr_reader
        .read_to_string(&mut stderr_acc)
        .await
        .context("read stderr")?;

    let status = child.wait().await.context("wait subprocess")?;
    let code = status.code().unwrap_or(1);
    let merged = crate::tools::driver::merge_shell_output(&stdout_acc, &stderr_acc);
    Ok((merged, code, all_events))
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
