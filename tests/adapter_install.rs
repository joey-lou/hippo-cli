use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn temp_home() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("hippo-adapter-{nanos}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn hippo(home: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_hippo"))
        .env("HOME", home)
        .env_remove("PI_AGENT_DIR")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

#[test]
fn list_ships_cursor_and_pi() {
    let home = temp_home();
    let output = hippo(&home, &["adapter", "list"]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text, "cursor\npi\n");
}

#[test]
fn install_cursor_links_files_and_keeps_other_hooks() {
    let home = temp_home();
    let hooks_json = home.join(".cursor").join("hooks.json");
    fs::create_dir_all(hooks_json.parent().unwrap()).unwrap();
    fs::write(
        &hooks_json,
        r#"{"version":1,"hooks":{"sessionStart":[{"command":"./hooks/herdr.sh"},{"command":"./hooks/hippo-session-start.sh"}]}}"#,
    )
    .unwrap();

    let output = hippo(&home, &["adapter", "install", "cursor"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let skill = home.join(".cursor/skills/use-memory/SKILL.md");
    assert!(skill.is_file(), "missing {}", skill.display());
    let script = home.join(".config/hippo/adapters/cursor/hooks/hippo-session-start.sh");
    assert!(script.is_file());
    let mode = fs::metadata(&script).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(mode.mode() & 0o111, 0o111);
    }

    let merged: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks_json).unwrap()).unwrap();
    let session = merged["hooks"]["sessionStart"].as_array().unwrap();
    let commands: Vec<&str> = session
        .iter()
        .filter_map(|item| item["command"].as_str())
        .collect();
    assert_eq!(
        commands,
        vec!["./hooks/herdr.sh", "./hooks/hippo-session-start.sh"]
    );
    assert!(merged["hooks"]["afterShellExecution"][0]["matcher"]
        .as_str()
        .unwrap()
        .contains("consolidate"));

    let again = hippo(&home, &["adapter", "install", "cursor"]);
    assert_eq!(again.status.code(), Some(0));
    let merged: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks_json).unwrap()).unwrap();
    assert_eq!(merged["hooks"]["sessionStart"].as_array().unwrap().len(), 2);
}

#[test]
fn install_pi_links_skill_and_extension() {
    let home = temp_home();
    let output = hippo(&home, &["adapter", "install", "pi"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(home.join(".pi/agent/skills/use-memory/SKILL.md").is_file());
    assert!(home.join(".pi/agent/extensions/hippo.ts").is_file());
}

#[test]
fn unknown_adapter_exits_2() {
    let home = temp_home();
    let output = hippo(&home, &["adapter", "install", "nope"]);
    assert_eq!(output.status.code(), Some(2));
    let err = String::from_utf8(output.stderr).unwrap();
    assert!(err.contains("unknown adapter"));
}

#[test]
fn reinstall_drops_links_and_hooks_the_new_adapter_no_longer_owns() {
    let home = temp_home();
    let stale = home.join(".cursor/hooks/hippo-removed.sh");
    fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink("/dev/null", &stale).unwrap();
    let hooks_json = home.join(".cursor/hooks.json");
    fs::write(
        &hooks_json,
        r#"{"version":1,"hooks":{"beforeSubmitPrompt":[{"command":"./hooks/hippo-removed.sh"}],"sessionStart":[{"command":"./hooks/herdr.sh"}]}}"#,
    )
    .unwrap();
    let manifest = home.join(".config/hippo/adapters/manifest.json");
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(
        &manifest,
        format!(
            r#"{{"cursor":{{"version":"0.0.0","links":["{}"],"hooks":[{{"file":"{}","event":"beforeSubmitPrompt","command":"./hooks/hippo-removed.sh"}}]}}}}"#,
            stale.display(),
            hooks_json.display()
        ),
    )
    .unwrap();

    let output = hippo(&home, &["adapter", "install", "cursor"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!stale.exists());
    let merged: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks_json).unwrap()).unwrap();
    assert!(merged["hooks"].get("beforeSubmitPrompt").is_none());
    assert!(merged["hooks"]["sessionStart"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["command"] == "./hooks/herdr.sh"));
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
    let links = written["cursor"]["links"].as_array().unwrap();
    assert!(links.iter().all(|link| link.as_str() != Some(stale.to_str().unwrap())));
    assert!(written["cursor"]["hooks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|hook| hook["command"] != "./hooks/hippo-removed.sh"));
    assert!(written["cursor"]["links"].as_array().unwrap().len() >= 6);
}

#[test]
fn manifest_keeps_other_adapters() {
    let home = temp_home();
    assert_eq!(hippo(&home, &["adapter", "install", "pi"]).status.code(), Some(0));
    assert_eq!(
        hippo(&home, &["adapter", "install", "cursor"]).status.code(),
        Some(0)
    );
    let manifest = home.join(".config/hippo/adapters/manifest.json");
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
    assert!(written.get("pi").is_some());
    assert!(written.get("cursor").is_some());
    assert_eq!(
        hippo(&home, &["adapter", "install", "cursor"]).status.code(),
        Some(0)
    );
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
    assert!(written["pi"]["links"].as_array().unwrap().len() >= 2);
    assert!(home.join(".pi/agent/extensions/hippo.ts").exists());
}

#[test]
fn install_refuses_a_path_owned_by_another_adapter() {
    let home = temp_home();
    let owned = home.join(".cursor/hooks/hippo-session-start.sh");
    let manifest = home.join(".config/hippo/adapters/manifest.json");
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(
        &manifest,
        format!(
            r#"{{"pi":{{"version":"0.0.0","links":["{}"],"hooks":[]}}}}"#,
            owned.display()
        ),
    )
    .unwrap();
    let output = hippo(&home, &["adapter", "install", "cursor"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr).unwrap().contains("owned by adapter 'pi'"));
    assert!(!owned.exists());
}

#[test]
fn broken_hooks_json_exits_2() {
    let home = temp_home();
    let hooks_json = home.join(".cursor/hooks.json");
    fs::create_dir_all(hooks_json.parent().unwrap()).unwrap();
    fs::write(&hooks_json, "not json").unwrap();
    let output = hippo(&home, &["adapter", "install", "cursor"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr).unwrap().contains("not valid JSON"));
}
