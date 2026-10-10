//! Adapters embedded in the `hippo` binary.
//!
//! Each adapter directory carries an `adapter.json` that lists the links and hook
//! entries it owns. Install writes those paths into one shared manifest and the
//! next install of that adapter removes the previous list before linking again.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use include_dir::{include_dir, Dir, DirEntry};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{HippoError, Result};

static ADAPTERS: Dir = include_dir!("$CARGO_MANIFEST_DIR/adapters");

const ROOTS: &[&str] = &["cursor", "pi"];

/// Adapter names shipped in this binary.
pub fn names() -> Result<Vec<String>> {
    Ok(load_specs()?.into_keys().collect())
}

/// Extract an adapter and link it into the agent config under `home`.
pub fn install(name: &str, home: &Path) -> Result<()> {
    let specs = load_specs()?;
    let Some(spec) = specs.get(name) else {
        return Err(HippoError::Validation(format!(
            "unknown adapter '{name}'. available: {}",
            specs.keys().cloned().collect::<Vec<_>>().join(", ")
        )));
    };
    println!("Installing Hippo {name} adapter…");
    let mut manifest = read_manifest(home)?;
    ensure_links_are_free(name, spec, home, &manifest)?;
    let removed = sweep(name, &manifest)?;
    if removed > 0 {
        println!("  removed: {removed} entries from the previous install");
    }
    let extracted = extract(name, home)?;
    let record = apply(name, spec, &extracted, home)?;
    manifest.insert(name.to_string(), record);
    let manifest_path = write_manifest(home, &manifest)?;
    println!("  manifest: {}", manifest_path.display());
    println!("{}", done_message(&spec.root));
    Ok(())
}

fn done_message(root: &str) -> &'static str {
    match root {
        "cursor" => "Done. Restart Cursor to activate.",
        "pi" => "Done. Start a new Pi session so the digest loads.",
        _ => "Done.",
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterSpec {
    root: String,
    links: Vec<AdapterLink>,
    hooks: Vec<AdapterHook>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterLink {
    from: String,
    to: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterHook {
    file: String,
    event: String,
    entry: Value,
}

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
struct InstalledAdapter {
    version: String,
    links: Vec<PathBuf>,
    hooks: Vec<InstalledHook>,
}

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
struct InstalledHook {
    file: PathBuf,
    event: String,
    command: String,
}

fn load_specs() -> Result<BTreeMap<String, AdapterSpec>> {
    let mut specs = BTreeMap::new();
    for dir in ADAPTERS.dirs() {
        let name = dir
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| HippoError::Unexpected("adapter directory has no name".into()))?
            .to_string();
        let bytes = adapter_json(dir).ok_or_else(|| {
            HippoError::Validation(format!("adapter '{name}' is missing adapter.json"))
        })?;
        specs.insert(name.clone(), parse_spec(&name, bytes, dir)?);
    }
    let mut claimed: BTreeMap<(String, String), String> = BTreeMap::new();
    for (name, spec) in &specs {
        for link in &spec.links {
            let key = (spec.root.clone(), link.to.clone());
            if let Some(other) = claimed.insert(key, name.clone()) {
                return Err(HippoError::Validation(format!(
                    "adapters '{other}' and '{name}' both link {} under root {}",
                    link.to, spec.root
                )));
            }
        }
    }
    Ok(specs)
}

fn adapter_json<'a>(dir: &'a Dir<'a>) -> Option<&'a [u8]> {
    for entry in dir.entries() {
        let DirEntry::File(file) = entry else {
            continue;
        };
        if file.path().file_name().and_then(|value| value.to_str()) == Some("adapter.json") {
            return Some(file.contents());
        }
    }
    None
}

fn parse_spec(name: &str, bytes: &[u8], dir: &Dir) -> Result<AdapterSpec> {
    let spec: AdapterSpec = serde_json::from_slice(bytes).map_err(|err| {
        HippoError::Validation(format!("adapter '{name}' adapter.json is invalid: {err}"))
    })?;
    if !ROOTS.contains(&spec.root.as_str()) {
        return Err(HippoError::Validation(format!(
            "adapter '{name}' root '{}' is unknown. available: {}",
            spec.root,
            ROOTS.join(", ")
        )));
    }
    if spec.links.is_empty() && spec.hooks.is_empty() {
        return Err(HippoError::Validation(format!(
            "adapter '{name}' declares nothing to install"
        )));
    }
    let mut tos = BTreeSet::new();
    for link in &spec.links {
        let from = relative_path(name, "from", &link.from)?;
        let to = relative_path(name, "to", &link.to)?;
        if !adapter_contains(dir, &from) {
            return Err(HippoError::Validation(format!(
                "adapter '{name}' link from '{}' is not in the adapter tree",
                link.from
            )));
        }
        if !tos.insert(to) {
            return Err(HippoError::Validation(format!(
                "adapter '{name}' links '{}' twice",
                link.to
            )));
        }
    }
    let mut hooks = BTreeSet::new();
    for hook in &spec.hooks {
        relative_path(name, "file", &hook.file)?;
        if hook.event.is_empty() {
            return Err(HippoError::Validation(format!(
                "adapter '{name}' has a hook with an empty event"
            )));
        }
        let command = hook_command(name, &hook.entry)?;
        if !hooks.insert((hook.file.clone(), hook.event.clone(), command)) {
            return Err(HippoError::Validation(format!(
                "adapter '{name}' repeats hook {} {}",
                hook.file, hook.event
            )));
        }
    }
    Ok(spec)
}

fn hook_command(name: &str, entry: &Value) -> Result<String> {
    let command = entry
        .get("command")
        .and_then(Value::as_str)
        .filter(|command| !command.is_empty())
        .ok_or_else(|| {
            HippoError::Validation(format!(
                "adapter '{name}' hook entry needs a command string"
            ))
        })?;
    if !command.contains("hippo-") {
        return Err(HippoError::Validation(format!(
            "adapter '{name}' hook command '{command}' must contain 'hippo-'"
        )));
    }
    if !entry.is_object() {
        return Err(HippoError::Validation(format!(
            "adapter '{name}' hook entry must be a JSON object"
        )));
    }
    Ok(command.to_string())
}

fn relative_path(name: &str, field: &str, value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    let invalid = value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            !matches!(component, Component::Normal(_) | Component::CurDir)
        });
    if invalid {
        return Err(HippoError::Validation(format!(
            "adapter '{name}' {field} '{value}' must be a relative path inside the adapter"
        )));
    }
    Ok(path.to_path_buf())
}

fn adapter_contains(dir: &Dir, rel: &Path) -> bool {
    fn walk(dir: &Dir, parts: &[&std::ffi::OsStr]) -> bool {
        if parts.is_empty() {
            return true;
        }
        for entry in dir.entries() {
            if entry.path().file_name() != Some(parts[0]) {
                continue;
            }
            return match entry {
                DirEntry::Dir(subdir) => walk(subdir, &parts[1..]),
                DirEntry::File(_) => parts.len() == 1,
            };
        }
        false
    }
    let parts: Vec<&std::ffi::OsStr> = rel
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value),
            _ => None,
        })
        .collect();
    !parts.is_empty() && walk(dir, &parts)
}

fn manifest_path(home: &Path) -> PathBuf {
    home.join(".config")
        .join("hippo")
        .join("adapters")
        .join("manifest.json")
}

fn read_manifest(home: &Path) -> Result<BTreeMap<String, InstalledAdapter>> {
    let path = manifest_path(home);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let text = fs::read_to_string(&path).map_err(HippoError::unexpected)?;
    serde_json::from_str(&text).map_err(|_| {
        HippoError::Validation(format!(
            "{} is not a valid adapter manifest. Fix or remove it and re-run.",
            path.display()
        ))
    })
}

fn write_manifest(home: &Path, manifest: &BTreeMap<String, InstalledAdapter>) -> Result<PathBuf> {
    let path = manifest_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(HippoError::unexpected)?;
    }
    let text = serde_json::to_string_pretty(manifest).map_err(HippoError::unexpected)?;
    fs::write(&path, format!("{text}\n")).map_err(HippoError::unexpected)?;
    Ok(path)
}

fn ensure_links_are_free(
    name: &str,
    spec: &AdapterSpec,
    home: &Path,
    manifest: &BTreeMap<String, InstalledAdapter>,
) -> Result<()> {
    let root = adapter_root(&spec.root, home)?;
    for link in &spec.links {
        let dest = root.join(&link.to);
        for (other, installed) in manifest {
            if other == name {
                continue;
            }
            if installed.links.iter().any(|path| path == &dest) {
                return Err(HippoError::Validation(format!(
                    "{} is owned by adapter '{other}'",
                    dest.display()
                )));
            }
        }
    }
    Ok(())
}

fn sweep(name: &str, manifest: &BTreeMap<String, InstalledAdapter>) -> Result<usize> {
    let Some(previous) = manifest.get(name) else {
        return Ok(0);
    };
    let mut removed = 0;
    for link in &previous.links {
        let Ok(meta) = fs::symlink_metadata(link) else {
            continue;
        };
        if !meta.file_type().is_symlink() {
            return Err(HippoError::Validation(format!(
                "{} was installed by adapter '{name}' and is no longer a symlink. Move it aside and re-run.",
                link.display()
            )));
        }
        fs::remove_file(link).map_err(HippoError::unexpected)?;
        removed += 1;
    }
    let mut by_file: BTreeMap<&Path, Vec<&InstalledHook>> = BTreeMap::new();
    for hook in &previous.hooks {
        by_file.entry(&hook.file).or_default().push(hook);
    }
    for (file, hooks) in by_file {
        if !file.exists() {
            continue;
        }
        remove_hooks(file, &hooks)?;
        removed += hooks.len();
    }
    Ok(removed)
}

fn remove_hooks(file: &Path, hooks: &[&InstalledHook]) -> Result<()> {
    let mut data = read_json_object(file)?;
    let root = data.as_object_mut().ok_or_else(|| {
        HippoError::Validation(format!("{} must be a JSON object.", file.display()))
    })?;
    let Some(events) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    for hook in hooks {
        let empty = {
            let Some(list) = events.get_mut(&hook.event).and_then(Value::as_array_mut) else {
                continue;
            };
            list.retain(|item| {
                item.get("command").and_then(Value::as_str) != Some(hook.command.as_str())
            });
            list.is_empty()
        };
        if empty {
            events.remove(&hook.event);
        }
    }
    write_json(file, &data)
}

fn apply(name: &str, spec: &AdapterSpec, extracted: &Path, home: &Path) -> Result<InstalledAdapter> {
    let root = adapter_root(&spec.root, home)?;
    let mut links = Vec::new();
    for link in &spec.links {
        let dest = root.join(&link.to);
        force_symlink(&extracted.join(&link.from), &dest)?;
        links.push(dest);
    }
    println!("  links: {}", links.len());
    let mut hooks = Vec::new();
    let mut by_file: BTreeMap<&str, Vec<&AdapterHook>> = BTreeMap::new();
    for hook in &spec.hooks {
        by_file.entry(&hook.file).or_default().push(hook);
    }
    for (file, entries) in by_file {
        let path = root.join(file);
        apply_hooks(&path, &entries)?;
        for hook in entries {
            hooks.push(InstalledHook {
                file: path.clone(),
                event: hook.event.clone(),
                command: hook_command(name, &hook.entry)?,
            });
        }
    }
    if !hooks.is_empty() {
        println!("  hooks: {}", hooks.len());
    }
    Ok(InstalledAdapter {
        version: env!("CARGO_PKG_VERSION").to_string(),
        links,
        hooks,
    })
}

fn apply_hooks(file: &Path, entries: &[&AdapterHook]) -> Result<()> {
    let mut data = if file.exists() {
        read_json_object(file)?
    } else {
        json!({})
    };
    let root = data.as_object_mut().ok_or_else(|| {
        HippoError::Validation(format!("{} must be a JSON object.", file.display()))
    })?;
    root.entry("version").or_insert(json!(1));
    let events = root.entry("hooks").or_insert(json!({}));
    let events = events.as_object_mut().ok_or_else(|| {
        HippoError::Validation(format!("{} hooks must be a JSON object.", file.display()))
    })?;
    for hook in entries {
        let command = hook
            .entry
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let list = events.entry(&hook.event).or_insert(json!([]));
        let list = list.as_array_mut().ok_or_else(|| {
            HippoError::Validation(format!("{} hooks must be a JSON array.", hook.event))
        })?;
        list.retain(|item| item.get("command").and_then(Value::as_str) != Some(command));
        list.push(hook.entry.clone());
    }
    write_json(file, &data)
}

fn read_json_object(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path).map_err(HippoError::unexpected)?;
    serde_json::from_str(&text).map_err(|_| {
        HippoError::Validation(format!(
            "{} is not valid JSON; fix or remove it and re-run.",
            path.display()
        ))
    })
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(HippoError::unexpected)?;
    }
    let text = serde_json::to_string_pretty(value).map_err(HippoError::unexpected)?;
    fs::write(path, format!("{text}\n")).map_err(HippoError::unexpected)?;
    Ok(())
}

fn adapter_root(root: &str, home: &Path) -> Result<PathBuf> {
    match root {
        "cursor" => Ok(home.join(".cursor")),
        "pi" => Ok(std::env::var_os("PI_AGENT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".pi").join("agent"))),
        other => Err(HippoError::Validation(format!(
            "unknown adapter root '{other}'"
        ))),
    }
}

fn extract(name: &str, home: &Path) -> Result<PathBuf> {
    let source = ADAPTERS.get_dir(name).ok_or_else(|| {
        HippoError::Validation(format!("adapter '{name}' is missing from this binary"))
    })?;
    let dest = home
        .join(".config")
        .join("hippo")
        .join("adapters")
        .join(name);
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

#[cfg(test)]
mod tests {
    use super::load_specs;

    #[test]
    fn shipped_adapters_satisfy_the_spec() {
        let specs = load_specs().unwrap();
        assert_eq!(specs.len(), 2);
        let cursor = &specs["cursor"];
        assert_eq!(cursor.root, "cursor");
        assert_eq!(cursor.links.len(), 4);
        assert_eq!(cursor.hooks.len(), 3);
        let pi = &specs["pi"];
        assert_eq!(pi.root, "pi");
        assert_eq!(pi.links.len(), 2);
        assert!(pi.hooks.is_empty());
    }
}
