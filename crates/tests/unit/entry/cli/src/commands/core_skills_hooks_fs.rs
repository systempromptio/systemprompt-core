//! Filesystem-driven tests for `core skills list/show` and `core hooks list`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use clap::Parser;
use systemprompt_cli::core::skills::list::{execute_with_path, show_skill_detail};
use systemprompt_cli::core::{hooks, skills};

#[derive(Debug, Parser)]
struct SkillsListHarness {
    #[command(flatten)]
    args: skills::list::ListArgs,
}

fn list_args(argv: &[&str]) -> skills::list::ListArgs {
    SkillsListHarness::try_parse_from(std::iter::once("list").chain(argv.iter().copied()))
        .unwrap()
        .args
}

fn write_skill(root: &Path, id: &str, enabled: bool, instructions: Option<&str>) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    let yaml = format!(
        "id: {id}\nname: {id} skill\ndescription: A {id} skill\nenabled: {enabled}\ntags: [t1]\n"
    );
    fs::write(dir.join("config.yaml"), yaml).unwrap();
    if let Some(text) = instructions {
        fs::write(dir.join("index.md"), text).unwrap();
    }
}

fn artifact_json(out: systemprompt_cli::shared::CommandOutput) -> String {
    serde_json::to_value(out.into_artifact())
        .unwrap()
        .to_string()
}

#[test]
fn skills_list_missing_dir_is_empty() {
    let out = execute_with_path(list_args(&[]), Path::new("/nonexistent/skills-root")).unwrap();
    let json = artifact_json(out);
    assert!(!json.contains("skill_id\":\""), "{json}");
}

#[test]
fn skills_list_filters_enabled_and_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    write_skill(tmp.path(), "on-skill", true, None);
    write_skill(tmp.path(), "off-skill", false, None);
    fs::write(tmp.path().join("stray.txt"), "x").unwrap();
    fs::create_dir_all(tmp.path().join("no-config")).unwrap();

    let all = artifact_json(execute_with_path(list_args(&[]), tmp.path()).unwrap());
    assert!(
        all.contains("on-skill") && all.contains("off-skill"),
        "{all}"
    );

    let enabled = artifact_json(execute_with_path(list_args(&["--enabled"]), tmp.path()).unwrap());
    assert!(
        enabled.contains("on-skill") && !enabled.contains("off-skill"),
        "{enabled}"
    );

    let disabled =
        artifact_json(execute_with_path(list_args(&["--disabled"]), tmp.path()).unwrap());
    assert!(
        disabled.contains("off-skill") && !disabled.contains("on-skill"),
        "{disabled}"
    );
}

#[test]
fn skills_list_skips_unparseable_config() {
    let tmp = tempfile::tempdir().unwrap();
    write_skill(tmp.path(), "good", true, None);
    let broken = tmp.path().join("broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("config.yaml"), "not: [valid: yaml").unwrap();

    let json = artifact_json(execute_with_path(list_args(&[]), tmp.path()).unwrap());
    assert!(json.contains("good") && !json.contains("broken"), "{json}");
}

#[test]
fn skills_list_with_name_renders_detail() {
    let tmp = tempfile::tempdir().unwrap();
    let long = "x".repeat(300);
    write_skill(tmp.path(), "detail", true, Some(&long));

    let json = artifact_json(execute_with_path(list_args(&["detail"]), tmp.path()).unwrap());
    assert!(json.contains("detail skill"), "{json}");
    assert!(json.contains('\u{2026}') || json.contains("..."), "{json}");
}

#[test]
fn skill_detail_unknown_name_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let err = show_skill_detail("ghost", tmp.path()).unwrap_err();
    assert!(err.to_string().contains("not found"), "{err}");
}

#[test]
fn skill_detail_without_config_errors() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("bare")).unwrap();
    let err = show_skill_detail("bare", tmp.path()).unwrap_err();
    assert!(err.to_string().contains("has no"), "{err}");
}

#[test]
fn skill_detail_without_content_file_has_empty_preview() {
    let tmp = tempfile::tempdir().unwrap();
    write_skill(tmp.path(), "nocontent", true, None);
    let json = artifact_json(show_skill_detail("nocontent", tmp.path()).unwrap());
    assert!(json.contains("nocontent skill"), "{json}");
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

const PLUGIN_YAML: &str = r#"
plugin:
  id: demo
  name: Demo Plugin
  description: A demo plugin
  version: 1.0.0
  author:
    name: Tester
    email: tester@example.com
  keywords: [demo]
  license: MIT
  category: tools
  skills:
    source: explicit
    include: [alpha_skill]
  agents:
    source: explicit
    include: [helper]
"#;

fn plugin_show(root: &Path, id: &str) -> anyhow::Result<systemprompt_cli::shared::CommandOutput> {
    let args = systemprompt_cli::core::plugins::show::ShowArgs { id: id.to_owned() };
    systemprompt_cli::core::plugins::show::execute_with_path(&args, root)
}

#[test]
fn plugin_show_renders_parsed_config() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("demo");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.yaml"), PLUGIN_YAML).unwrap();

    let json = artifact_json(plugin_show(tmp.path(), "demo").unwrap());
    assert!(json.contains("Demo Plugin"), "{json}");
    assert!(json.contains("alpha_skill"), "{json}");
    assert!(json.contains("Tester"), "{json}");
}

#[test]
fn plugin_show_unknown_id_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let err = plugin_show(tmp.path(), "ghost").unwrap_err();
    assert!(err.to_string().contains("not found"), "{err}");
}

#[test]
fn plugin_show_without_config_errors() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("bare")).unwrap();
    let err = plugin_show(tmp.path(), "bare").unwrap_err();
    assert!(err.to_string().contains("no config.yaml"), "{err}");
}
