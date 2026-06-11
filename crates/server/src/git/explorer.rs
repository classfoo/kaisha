use super::service::run_git_ok;
use serde::Serialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const BINARY_SNIFF_BYTES: usize = 8000;

#[derive(Debug, Clone, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub current: bool,
    pub remote: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BranchList {
    pub current: String,
    pub branches: Vec<BranchInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TreeListing {
    pub path: String,
    pub entries: Vec<TreeEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub size: u64,
    pub binary: bool,
    pub truncated: bool,
}

pub fn list_branches(repo_dir: &Path) -> anyhow::Result<BranchList> {
    if !repo_dir.join(".git").exists() {
        anyhow::bail!("git_not_initialized");
    }
    let current = run_git_ok(
        repo_dir,
        &["rev-parse".into(), "--abbrev-ref".into(), "HEAD".into()],
    )
    .unwrap_or_else(|_| "HEAD".to_string());

    let mut branches = Vec::new();

    let locals = run_git_ok(
        repo_dir,
        &[
            "for-each-ref".into(),
            "--format=%(refname:short)".into(),
            "refs/heads".into(),
        ],
    )
    .unwrap_or_default();
    for name in locals.lines().map(str::trim).filter(|s| !s.is_empty()) {
        branches.push(BranchInfo {
            name: name.to_string(),
            current: name == current,
            remote: false,
        });
    }

    let remotes = run_git_ok(
        repo_dir,
        &[
            "for-each-ref".into(),
            "--format=%(refname:short)".into(),
            "refs/remotes".into(),
        ],
    )
    .unwrap_or_default();
    for name in remotes.lines().map(str::trim).filter(|s| !s.is_empty()) {
        // Skip symbolic refs such as `origin/HEAD`.
        if name.ends_with("/HEAD") {
            continue;
        }
        branches.push(BranchInfo {
            name: name.to_string(),
            current: false,
            remote: true,
        });
    }

    Ok(BranchList { current, branches })
}

/// Resolve a user-supplied relative path against the repo root, rejecting
/// traversal attempts and access to the internal `.git` directory.
fn safe_join(repo_dir: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let rel = rel.trim().trim_start_matches('/');
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        anyhow::bail!("git_path_invalid");
    }
    for comp in rel_path.components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(part) => {
                if part == ".git" {
                    anyhow::bail!("git_path_invalid");
                }
            }
            _ => anyhow::bail!("git_path_invalid"),
        }
    }
    let joined = repo_dir.join(rel_path);
    let repo_canon = repo_dir
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("git_path_not_found"))?;
    let target_canon = joined
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("git_path_not_found"))?;
    if !target_canon.starts_with(&repo_canon) {
        anyhow::bail!("git_path_invalid");
    }
    Ok(target_canon)
}

fn rel_display(repo_canon: &Path, target: &Path, name: &str) -> String {
    match target.strip_prefix(repo_canon) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => name.to_string(),
    }
}

pub fn list_tree(repo_dir: &Path, rel: &str) -> anyhow::Result<TreeListing> {
    let target = safe_join(repo_dir, rel)?;
    if !target.is_dir() {
        anyhow::bail!("git_path_not_directory");
    }
    let repo_canon = repo_dir
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("git_path_not_found"))?;

    let mut entries: Vec<TreeEntry> = Vec::new();
    for dir_entry in fs::read_dir(&target)? {
        let dir_entry = dir_entry?;
        let name = dir_entry.file_name().to_string_lossy().to_string();
        if name == ".git" {
            continue;
        }
        let path = dir_entry.path();
        let meta = match dir_entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let is_dir = meta.is_dir();
        entries.push(TreeEntry {
            name: name.clone(),
            path: rel_display(&repo_canon, &path, &name),
            is_dir,
            size: if is_dir { 0 } else { meta.len() },
        });
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    let listing_path = rel_display(&repo_canon, &target, "");
    Ok(TreeListing {
        path: listing_path,
        entries,
    })
}

pub fn read_file(repo_dir: &Path, rel: &str) -> anyhow::Result<FileContent> {
    let target = safe_join(repo_dir, rel)?;
    if !target.is_file() {
        anyhow::bail!("git_path_not_file");
    }
    let repo_canon = repo_dir
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("git_path_not_found"))?;
    let display_path = rel_display(&repo_canon, &target, "");

    let meta = fs::metadata(&target)?;
    let size = meta.len();
    let bytes = fs::read(&target)?;

    let sniff_len = bytes.len().min(BINARY_SNIFF_BYTES);
    let binary = bytes[..sniff_len].contains(&0);
    if binary {
        return Ok(FileContent {
            path: display_path,
            content: String::new(),
            size,
            binary: true,
            truncated: false,
        });
    }

    let truncated = size > MAX_FILE_BYTES;
    let slice = if truncated {
        &bytes[..MAX_FILE_BYTES as usize]
    } else {
        &bytes[..]
    };
    let content = String::from_utf8_lossy(slice).to_string();

    Ok(FileContent {
        path: display_path,
        content,
        size,
        binary: false,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::service::{git_init, run_git_ok};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir_all(&dir).expect("create repo dir");
        git_init(&dir).expect("git init");
        dir
    }

    #[test]
    fn list_branches_reports_current() {
        let repo = temp_repo("kaisha-git-branches");
        fs::write(repo.join("a.txt"), "hello").unwrap();
        run_git_ok(&repo, &["add".into(), "-A".into()]).unwrap();
        run_git_ok(&repo, &["config".into(), "user.email".into(), "t@t.dev".into()]).unwrap();
        run_git_ok(&repo, &["config".into(), "user.name".into(), "t".into()]).unwrap();
        run_git_ok(&repo, &["commit".into(), "-m".into(), "init".into()]).unwrap();

        let list = list_branches(&repo).expect("branches");
        assert!(list.branches.iter().any(|b| b.current));
        assert!(!list.current.is_empty());
    }

    #[test]
    fn list_tree_sorts_dirs_first_and_hides_git() {
        let repo = temp_repo("kaisha-git-tree");
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(repo.join("readme.md"), "# hi").unwrap();

        let listing = list_tree(&repo, "").expect("tree");
        let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&".git"));
        assert_eq!(names.first().copied(), Some("src"));
        assert!(names.contains(&"readme.md"));

        let sub = list_tree(&repo, "src").expect("subtree");
        assert_eq!(sub.entries.len(), 1);
        assert_eq!(sub.entries[0].name, "main.rs");
        assert_eq!(sub.entries[0].path, "src/main.rs");
    }

    #[test]
    fn read_file_returns_content() {
        let repo = temp_repo("kaisha-git-readfile");
        fs::write(repo.join("hello.txt"), "hello world").unwrap();
        let file = read_file(&repo, "hello.txt").expect("read");
        assert_eq!(file.content, "hello world");
        assert!(!file.binary);
        assert!(!file.truncated);
    }

    #[test]
    fn read_file_detects_binary() {
        let repo = temp_repo("kaisha-git-binary");
        fs::write(repo.join("blob.bin"), [0u8, 1, 2, 3, 0]).unwrap();
        let file = read_file(&repo, "blob.bin").expect("read");
        assert!(file.binary);
        assert!(file.content.is_empty());
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let repo = temp_repo("kaisha-git-traversal");
        assert!(read_file(&repo, "../secret").is_err());
        assert!(list_tree(&repo, "../").is_err());
        assert!(read_file(&repo, ".git/config").is_err());
    }
}
