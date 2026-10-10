use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::error::{HippoError, Result};

const COMMIT_MESSAGE: &str = "Record memory updates.";

#[derive(Debug, Clone, Serialize)]
pub struct GitStatus {
    pub is_repo: bool,
    pub dirty: bool,
    pub ahead: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SyncReport {
    pub committed: bool,
    pub pushed: bool,
}

pub fn status(path: &Path) -> GitStatus {
    if !is_repo(path) {
        return GitStatus {
            is_repo: false,
            dirty: false,
            ahead: 0,
        };
    }
    let dirty =
        git(path, &["status", "--porcelain"]).is_some_and(|output| !output.trim().is_empty());
    GitStatus {
        is_repo: true,
        dirty,
        ahead: ahead(path),
    }
}

/// Commit `memory_dir` and push it when the branch has an upstream.
/// A home outside git is left alone.
pub fn sync(home: &Path, memory_dir: &Path) -> Result<SyncReport> {
    if !is_repo(home) || !memory_dir.is_dir() {
        return Ok(SyncReport::default());
    }
    let pathspec = memory_dir.to_string_lossy();
    run(home, &["add", "-A", "--", &pathspec])?;
    let committed = !succeeds(home, &["diff", "--cached", "--quiet"])?;
    if committed {
        run(home, &["commit", "-q", "-m", COMMIT_MESSAGE])?;
    }
    let pushed = ahead(home) > 0;
    if pushed {
        run(home, &["push", "-q"])?;
    }
    Ok(SyncReport { committed, pushed })
}

fn is_repo(path: &Path) -> bool {
    git(path, &["rev-parse", "--is-inside-work-tree"]).is_some()
}

fn ahead(path: &Path) -> i64 {
    git(path, &["rev-list", "--count", "@{u}..HEAD"])
        .and_then(|output| output.trim().parse().ok())
        .unwrap_or(0)
}

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let output = command(path, args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn succeeds(path: &Path, args: &[&str]) -> Result<bool> {
    let status = command(path, args)
        .status()
        .map_err(HippoError::unexpected)?;
    Ok(status.success())
}

fn run(path: &Path, args: &[&str]) -> Result<()> {
    let output = command(path, args)
        .output()
        .map_err(HippoError::unexpected)?;
    if output.status.success() {
        return Ok(());
    }
    Err(HippoError::Unexpected(format!(
        "git {} failed: {}",
        args[0],
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

fn command(path: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(path).args(args);
    command
}
