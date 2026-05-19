use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    process::{Command, Output},
};

#[derive(Debug, Clone, Serialize)]
pub struct GitCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum GitOperation {
    Status,
    Add {
        paths: Vec<String>,
        all: Option<bool>,
    },
    Reset {
        mode: Option<String>,
        paths: Vec<String>,
    },
    Commit {
        message: String,
        all: Option<bool>,
    },
    Log {
        max_count: Option<u32>,
        oneline: Option<bool>,
    },
    Branch {
        name: Option<String>,
        delete: Option<bool>,
        list: Option<bool>,
    },
    Checkout {
        target: String,
        create: Option<bool>,
    },
    Merge {
        branch: String,
    },
    Pull {
        remote: Option<String>,
        branch: Option<String>,
    },
    Push {
        remote: Option<String>,
        branch: Option<String>,
        set_upstream: Option<bool>,
    },
    Fetch {
        remote: Option<String>,
        prune: Option<bool>,
    },
    Remote {
        name: Option<String>,
        url: Option<String>,
        remove: Option<bool>,
        list: Option<bool>,
    },
    Clone {
        url: String,
        directory: Option<String>,
    },
    Diff {
        cached: Option<bool>,
        paths: Vec<String>,
    },
    Stash {
        action: String,
        message: Option<String>,
    },
    Tag {
        name: String,
        message: Option<String>,
        delete: Option<bool>,
        list: Option<bool>,
    },
    Raw {
        args: Vec<String>,
    },
}

fn run_git_raw(repo_dir: &Path, args: &[String]) -> anyhow::Result<Output> {
    if args.is_empty() {
        anyhow::bail!("git_args_empty");
    }
    if args.iter().any(|a| a == "-c" || a == "--config") {
        anyhow::bail!("git_config_injection_denied");
    }
    let output = Command::new("git")
        .current_dir(repo_dir)
        .args(args)
        .output()?;
    Ok(output)
}

pub fn run_git(repo_dir: &Path, args: &[String]) -> anyhow::Result<GitCommandOutput> {
    let output = run_git_raw(repo_dir, args)?;
    Ok(GitCommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

pub fn run_git_ok(repo_dir: &Path, args: &[String]) -> anyhow::Result<String> {
    let result = run_git(repo_dir, args)?;
    if result.exit_code == 0 {
        return Ok(result.stdout.trim().to_string());
    }
    let msg = if result.stderr.trim().is_empty() {
        result.stdout.trim().to_string()
    } else {
        result.stderr.trim().to_string()
    };
    if msg.is_empty() {
        anyhow::bail!("git_command_failed");
    }
    anyhow::bail!(msg)
}

pub fn git_init(repo_dir: &Path) -> anyhow::Result<()> {
    run_git_ok(repo_dir, &[String::from("init")])?;
    Ok(())
}

pub fn operation_to_args(op: &GitOperation) -> Vec<String> {
    match op {
        GitOperation::Status => vec!["status".into(), "--short".into(), "--branch".into()],
        GitOperation::Add { paths, all } => {
            let mut args = vec!["add".into()];
            if all.unwrap_or(false) {
                args.push("-A".into());
            } else if paths.is_empty() {
                args.push("-A".into());
            } else {
                args.extend(paths.clone());
            }
            args
        }
        GitOperation::Reset { mode, paths } => {
            let mut args = vec!["reset".into()];
            if let Some(m) = mode {
                args.push(m.clone());
            }
            args.extend(paths.clone());
            args
        }
        GitOperation::Commit { message, all } => {
            let mut args = vec!["commit".into(), "-m".into(), message.clone()];
            if all.unwrap_or(false) {
                args.insert(1, "-a".into());
            }
            args
        }
        GitOperation::Log {
            max_count,
            oneline,
        } => {
            let mut args = vec!["log".into()];
            if oneline.unwrap_or(true) {
                args.push("--oneline".into());
            }
            if let Some(n) = max_count {
                args.push(format!("-n{n}"));
            } else {
                args.push("-n".into());
                args.push("30".into());
            }
            args
        }
        GitOperation::Branch { name, delete, list } => {
            if list.unwrap_or(false) || name.is_none() {
                return vec!["branch".into(), "-a".into(), "--verbose".into()];
            }
            if delete.unwrap_or(false) {
                return vec![
                    "branch".into(),
                    "-d".into(),
                    name.clone().unwrap_or_default(),
                ];
            }
            vec!["branch".into(), name.clone().unwrap_or_default()]
        }
        GitOperation::Checkout { target, create } => {
            if create.unwrap_or(false) {
                vec!["checkout".into(), "-b".into(), target.clone()]
            } else {
                vec!["checkout".into(), target.clone()]
            }
        }
        GitOperation::Merge { branch } => vec!["merge".into(), branch.clone()],
        GitOperation::Pull { remote, branch } => {
            let mut args = vec!["pull".into()];
            if let Some(r) = remote {
                args.push(r.clone());
            }
            if let Some(b) = branch {
                args.push(b.clone());
            }
            args
        }
        GitOperation::Push {
            remote,
            branch,
            set_upstream,
        } => {
            let mut args = vec!["push".into()];
            if set_upstream.unwrap_or(false) {
                args.push("-u".into());
            }
            if let Some(r) = remote {
                args.push(r.clone());
            }
            if let Some(b) = branch {
                args.push(b.clone());
            }
            args
        }
        GitOperation::Fetch { remote, prune } => {
            let mut args = vec!["fetch".into(), "--all".into()];
            if prune.unwrap_or(false) {
                args.push("--prune".into());
            }
            if let Some(r) = remote {
                args = vec!["fetch".into(), r.clone()];
                if prune.unwrap_or(false) {
                    args.push("--prune".into());
                }
            }
            args
        }
        GitOperation::Remote {
            name,
            url,
            remove,
            list,
        } => {
            if list.unwrap_or(false) || (name.is_none() && url.is_none()) {
                return vec!["remote".into(), "-v".into()];
            }
            if remove.unwrap_or(false) {
                return vec!["remote".into(), "remove".into(), name.clone().unwrap_or_default()];
            }
            vec![
                "remote".into(),
                "add".into(),
                name.clone().unwrap_or_default(),
                url.clone().unwrap_or_default(),
            ]
        }
        GitOperation::Clone { url, directory } => {
            let mut args = vec!["clone".into(), url.clone()];
            if let Some(dir) = directory {
                args.push(dir.clone());
            }
            args
        }
        GitOperation::Diff { cached, paths } => {
            let mut args = vec!["diff".into()];
            if cached.unwrap_or(false) {
                args.push("--cached".into());
            }
            args.extend(paths.clone());
            args
        }
        GitOperation::Stash { action, message } => match action.as_str() {
            "list" => vec!["stash".into(), "list".into()],
            "pop" => vec!["stash".into(), "pop".into()],
            "apply" => vec!["stash".into(), "apply".into()],
            "drop" => vec!["stash".into(), "drop".into()],
            _ => {
                let mut args = vec!["stash".into(), "push".into()];
                if let Some(m) = message {
                    args.push("-m".into());
                    args.push(m.clone());
                }
                args
            }
        },
        GitOperation::Tag { name, message, delete, list } => {
            if list.unwrap_or(false) {
                return vec!["tag".into(), "-l".into()];
            }
            if delete.unwrap_or(false) {
                return vec!["tag".into(), "-d".into(), name.clone()];
            }
            if let Some(m) = message {
                vec!["tag".into(), "-a".into(), name.clone(), "-m".into(), m.clone()]
            } else {
                vec!["tag".into(), name.clone()]
            }
        }
        GitOperation::Raw { args } => args.clone(),
    }
}

pub fn execute_operation(repo_dir: &Path, op: &GitOperation) -> anyhow::Result<GitCommandOutput> {
    let args = operation_to_args(op);
    run_git(repo_dir, &args)
}

#[derive(Debug, Clone, Serialize)]
pub struct GitRepoStatus {
    pub branch: String,
    pub clean: bool,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub porcelain: String,
}

pub fn repo_status(repo_dir: &Path) -> anyhow::Result<GitRepoStatus> {
    if !repo_dir.join(".git").exists() {
        anyhow::bail!("git_not_initialized");
    }
    let branch = run_git_ok(
        repo_dir,
        &[
            "rev-parse".into(),
            "--abbrev-ref".into(),
            "HEAD".into(),
        ],
    )
    .unwrap_or_else(|_| "HEAD".to_string());

    let porcelain = run_git_ok(
        repo_dir,
        &["status".into(), "--porcelain".into(), "-b".into()],
    )
    .unwrap_or_default();

    let mut staged = 0u32;
    let mut unstaged = 0u32;
    let mut untracked = 0u32;
    let mut ahead = 0u32;
    let mut behind = 0u32;

    for line in porcelain.lines() {
        if line.starts_with("##") {
            if let Some(rest) = line.strip_prefix("## ") {
                if let Some(ab) = rest.split_once(" [") {
                    let tracking = ab.0;
                    if let Some(idx) = tracking.find("ahead ") {
                        ahead = tracking[idx + 6..]
                            .split_whitespace()
                            .next()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                    }
                    if let Some(idx) = tracking.find("behind ") {
                        behind = tracking[idx + 7..]
                            .split_whitespace()
                            .next()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                    }
                }
            }
            continue;
        }
        if line.starts_with("??") {
            untracked += 1;
            continue;
        }
        if line.len() < 2 {
            continue;
        }
        let index = line.chars().next().unwrap_or(' ');
        let worktree = line.chars().nth(1).unwrap_or(' ');
        if index != ' ' && index != '?' {
            staged += 1;
        }
        if worktree != ' ' && worktree != '?' {
            unstaged += 1;
        }
    }

    let clean = staged == 0 && unstaged == 0 && untracked == 0;
    Ok(GitRepoStatus {
        branch,
        clean,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
        porcelain,
    })
}
