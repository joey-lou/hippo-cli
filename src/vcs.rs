use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GitStatus {
    pub is_repo: bool,
    pub dirty: bool,
    pub ahead: i64,
}

pub fn status(path: &Path) -> GitStatus {
    if git(path, &["rev-parse", "--is-inside-work-tree"]).is_none() {
        return GitStatus {
            is_repo: false,
            dirty: false,
            ahead: 0,
        };
    }
    let dirty =
        git(path, &["status", "--porcelain"]).is_some_and(|output| !output.trim().is_empty());
    let ahead = git(path, &["rev-list", "--count", "@{u}..HEAD"])
        .and_then(|output| output.trim().parse().ok())
        .unwrap_or(0);
    GitStatus {
        is_repo: true,
        dirty,
        ahead,
    }
}

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}
