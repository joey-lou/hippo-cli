//! Adapters embedded in the `hippo` binary.

use std::fs;
use std::path::{Path, PathBuf};

use include_dir::{include_dir, Dir, DirEntry};
use serde_json::{json, Value};

use crate::error::{HippoError, Result};

static ADAPTERS: Dir = include_dir!("$CARGO_MANIFEST_DIR/adapters");

const CURSOR_HOOKS: &[&str] = &[
    "hippo-session-start.sh",
    "hippo-after-file-edit.sh",
    "hippo-after-shell.sh",
    "hippo-sync-memory.sh",
    "hippo-stop.sh",
];

/// Adapter names shipped in this binary.
pub fn names() -> Vec<&'static str> {
    let mut names: Vec<&str> = ADAPTERS
        .dirs()
        .filter_map(|dir| dir.path().file_name()?.to_str())
        .collect();
    names.sort_unstable();
    names
}

/// Extract an adapter and link it into the agent config under `home`.
pub fn install(name: &str, home: &Path) -> Result<()> {
    match name {
        "cursor" => install_cursor(home),
        "pi" => install_pi(home),
        _ => Err(HippoError::Validation(format!(
            "unknown adapter '{name}'. available: {}",
            names().join(", ")
        ))),
    }
}

fn install_cursor(home: &Path) -> Result<()> {
    println!("Installing Hippo Cursor adapter…");
    let extracted = extract("cursor", home)?;
    let cursor = home.join(".cursor");
    let hooks = cursor.join("hooks");
    fs::create_dir_all(&hooks).map_err(HippoError::unexpected)?;
    for script in CURSOR_HOOKS {
        force_symlink(&extracted.join("hooks").join(script), &hooks.join(script))?;
    }
    println!("  hooks: symlinked into {}", hooks.display());
    merge_cursor_hooks(&cursor.join("hooks.json"))?;
    println!("  hooks.json: merged (existing hooks preserved)");
    let skills = cursor.join("skills");
    fs::create_dir_all(&skills).map_err(HippoError::unexpected)?;
    force_symlink(
        &extracted.join("skills").join("use-memory"),
        &skills.join("use-memory"),
    )?;
    println!("  skill: symlinked use-memory into {}", skills.display());
    println!("Done. Restart Cursor to activate.");
    Ok(())
}

fn install_pi(home: &Path) -> Result<()> {
    println!("Installing Hippo Pi adapter…");
    let extracted = extract("pi", home)?;
    let agent = std::env::var_os("PI_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".pi").join("agent"));
    let skills = agent.join("skills");
    let extensions = agent.join("extensions");
    fs::create_dir_all(&skills).map_err(HippoError::unexpected)?;
    fs::create_dir_all(&extensions).map_err(HippoError::unexpected)?;
    force_symlink(
        &extracted.join("skills").join("use-memory"),
        &skills.join("use-memory"),
    )?;
    force_symlink(
        &extracted.join("extensions").join("hippo.ts"),
        &extensions.join("hippo.ts"),
    )?;
    println!("  skill: {}", skills.join("use-memory").display());
    println!("  extension: {}", extensions.join("hippo.ts").display());
    println!("Done. Start a new Pi session so the digest loads.");
    Ok(())
}

fn extract(name: &str, home: &Path) -> Result<PathBuf> {
    let source = ADAPTERS.get_dir(name).ok_or_else(|| {
        HippoError::Validation(format!("adapter '{name}' is missing from this binary"))
    })?;
    let dest = home.join(".config").join("hippo").join("adapters").join(name);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(HippoError::unexpected)?;
    }
    write_tree(source, &dest)?;
    mark_scripts(&dest)?;
    println!("  files: {}", dest.display());
    Ok(dest)
}

fn write_tree(dir: &Dir, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(HippoError::unexpected)?;
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(subdir) => {
                let name = subdir
                    .path()
                    .file_name()
                    .ok_or_else(|| HippoError::Unexpected("adapter directory has no name".into()))?;
                write_tree(subdir, &dest.join(name))?;
            }
            DirEntry::File(file) => {
                let name = file
                    .path()
                    .file_name()
                    .ok_or_else(|| HippoError::Unexpected("adapter file has no name".into()))?;
                fs::write(dest.join(name), file.contents()).map_err(HippoError::unexpected)?;
            }
        }
    }
    Ok(())
}

fn mark_scripts(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(HippoError::unexpected)? {
        let entry = entry.map_err(HippoError::unexpected)?;
        let path = entry.path();
        if path.is_dir() {
            mark_scripts(&path)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("sh") {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path)
                    .map_err(HippoError::unexpected)?
                    .permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms).map_err(HippoError::unexpected)?;
            }
        }
    }
    Ok(())
}

fn merge_cursor_hooks(path: &Path) -> Result<()> {
    let mut data = if path.exists() {
        let text = fs::read_to_string(path).map_err(HippoError::unexpected)?;
        serde_json::from_str::<Value>(&text).map_err(|_| {
            HippoError::Validation(format!(
                "{} is not valid JSON; fix or remove it and re-run.",
                path.display()
            ))
        })?
    } else {
        json!({})
    };
    let root = data.as_object_mut().ok_or_else(|| {
        HippoError::Validation(format!("{} must be a JSON object.", path.display()))
    })?;
    root.entry("version").or_insert(json!(1));
    let hooks = root.entry("hooks").or_insert(json!({}));
    let hooks = hooks.as_object_mut().ok_or_else(|| {
        HippoError::Validation(format!("{} hooks must be a JSON object.", path.display()))
    })?;
    for (event, entry) in cursor_hook_entries() {
        let existing = hooks.entry(event).or_insert(json!([]));
        let list = existing.as_array_mut().ok_or_else(|| {
            HippoError::Validation(format!("{event} hooks must be a JSON array."))
        })?;
        list.retain(|item| {
            item.get("command")
                .and_then(Value::as_str)
                .map(|command| !command.contains("hippo-"))
                .unwrap_or(true)
        });
        list.push(entry);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(HippoError::unexpected)?;
    }
    let text = serde_json::to_string_pretty(&data).map_err(HippoError::unexpected)?;
    fs::write(path, format!("{text}\n")).map_err(HippoError::unexpected)?;
    Ok(())
}

fn cursor_hook_entries() -> Vec<(&'static str, Value)> {
    vec![
        (
            "sessionStart",
            json!({"command": "./hooks/hippo-session-start.sh", "timeout": 20}),
        ),
        (
            "afterFileEdit",
            json!({"command": "./hooks/hippo-after-file-edit.sh", "timeout": 30}),
        ),
        (
            "afterShellExecution",
            json!({
                "command": "./hooks/hippo-after-shell.sh",
                "matcher": "hippo\\s+(add|update|consolidate)\\b",
                "timeout": 30
            }),
        ),
        (
            "stop",
            json!({"command": "./hooks/hippo-stop.sh", "timeout": 15, "loop_limit": 1}),
        ),
    ]
}

fn force_symlink(target: &Path, link: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(link) {
        if meta.is_dir() && !meta.file_type().is_symlink() {
            return Err(HippoError::Validation(format!(
                "{} exists and is not a symlink. Move it aside and re-run.",
                link.display()
            )));
        }
        fs::remove_file(link).map_err(HippoError::unexpected)?;
    }
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent).map_err(HippoError::unexpected)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).map_err(HippoError::unexpected)?;
        return Ok(());
    }
    #[cfg(not(unix))]
    {
        let _ = (target, link);
        Err(HippoError::Validation(
            "adapter install links files and is supported on macOS and Linux.".into(),
        ))
    }
}
