//! Filesystem-driven tests for `core hooks list`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::core::hooks;

fn artifact_json(out: systemprompt_extension::cli::CommandOutput) -> String {
    serde_json::to_value(out.into_artifact())
        .unwrap()
        .to_string()
}

fn write_hook(root: &Path, dir: &str, yaml: &str) {
    let hook_dir = root.join(dir);
    fs::create_dir_all(&hook_dir).unwrap();
    fs::write(hook_dir.join("config.yaml"), yaml).unwrap();
}

fn hooks_list(root: &Path) -> String {
    artifact_json(hooks::list::execute_with_path(hooks::list::ListArgs, root).unwrap())
}

#[test]
fn hooks_list_missing_dir_is_empty() {
    let json = artifact_json(
        hooks::list::execute_with_path(hooks::list::ListArgs, Path::new("/nonexistent/hooks"))
            .unwrap(),
    );
    assert!(!json.contains("plugin_id\":\""), "{json}");
}

#[test]
fn hooks_list_uses_config_id_over_dir_name() {
    let tmp = tempfile::tempdir().unwrap();
    write_hook(
        tmp.path(),
        "dir-name",
        "id: my_hook\nevent: PreToolUse\ncommand: run.sh\n",
    );

    let json = hooks_list(tmp.path());
    assert!(
        json.contains("my_hook") && !json.contains("dir-name"),
        "{json}"
    );
    assert!(json.contains("run.sh"), "{json}");
}

#[test]
fn hooks_list_falls_back_to_dir_name_and_omits_empty_command() {
    let tmp = tempfile::tempdir().unwrap();
    write_hook(
        tmp.path(),
        "fallback-hook",
        "id: \"\"\nevent: PostToolUse\ncommand: \"\"\n",
    );

    let json = hooks_list(tmp.path());
    assert!(json.contains("fallback-hook"), "{json}");
    assert!(json.contains("PostToolUse"), "{json}");
}

#[test]
fn hooks_list_skips_broken_configs_and_stray_files() {
    let tmp = tempfile::tempdir().unwrap();
    write_hook(tmp.path(), "ok", "id: ok\nevent: PreToolUse\ncommand: c\n");
    write_hook(tmp.path(), "broken", "not: [valid: yaml");
    fs::write(tmp.path().join("stray.txt"), "x").unwrap();

    let json = hooks_list(tmp.path());
    assert!(json.contains("\"ok\""), "{json}");
    assert!(!json.contains("broken"), "{json}");
}
