//! Filesystem-driven tests for `core skills list/show`, skill config parsing
//! and `core plugins show`, each over an explicit services-tree path.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use clap::Parser;
use systemprompt_marketplace_entry::cli::skills;
use systemprompt_marketplace_entry::cli::skills::list::{execute_with_path, show_skill_detail};
use systemprompt_marketplace_entry::cli::skills::types::parse_skill_from_config;
use systemprompt_identifiers::SkillId;

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

fn artifact_json(out: systemprompt_extension::cli::CommandOutput) -> String {
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
fn skills_list_sorts_directory_ids_and_preserves_summary_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    for (id, name, enabled) in [
        ("zeta", "Zeta Skill", false),
        ("alpha", "Alpha Skill", true),
    ] {
        let dir = tmp.path().join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("config.yaml"),
            format!(
                "id: configured-{id}\nname: {name}\ndescription: {id} description\nenabled: {enabled}\ntags: [operator, review]\ncategory: governance\n"
            ),
        )
        .unwrap();
    }

    let json = artifact_json(execute_with_path(list_args(&[]), tmp.path()).unwrap());
    let alpha = json.find("alpha").unwrap();
    let zeta = json.find("zeta").unwrap();
    assert!(
        alpha < zeta,
        "skills must be ordered by directory ID: {json}"
    );
    assert!(json.contains("Alpha Skill"), "{json}");
    assert!(
        json.contains("operator") && json.contains("review"),
        "{json}"
    );
    assert!(
        json.contains("config.yaml"),
        "the source config path is displayed: {json}"
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
    let err = show_skill_detail(&SkillId::new("ghost"), tmp.path()).unwrap_err();
    assert!(err.to_string().contains("not found"), "{err}");
}

#[test]
fn skill_detail_without_config_errors() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("bare")).unwrap();
    let err = show_skill_detail(&SkillId::new("bare"), tmp.path()).unwrap_err();
    assert!(err.to_string().contains("has no"), "{err}");
}

#[test]
fn skill_detail_without_content_file_has_empty_preview() {
    let tmp = tempfile::tempdir().unwrap();
    write_skill(tmp.path(), "nocontent", true, None);
    let json = artifact_json(show_skill_detail(&SkillId::new("nocontent"), tmp.path()).unwrap());
    assert!(json.contains("nocontent skill"), "{json}");
}

#[test]
fn a_skill_is_parsed_from_its_config_and_content_file() {
    let dir = tempfile::tempdir().unwrap();
    let skill = dir.path().join("covskill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("config.yaml"),
        "id: covskill\nname: Coverage Skill\ndescription: Parsed from disk\nenabled: \
         true\ntags:\n  - alpha\n  - beta\ncategory: tools\n",
    )
    .unwrap();
    std::fs::write(
        skill.join("index.md"),
        "---\ntitle: ignored frontmatter\n---\n\nThe real instructions.\n",
    )
    .unwrap();

    let parsed = parse_skill_from_config(&skill.join("config.yaml"), &skill).unwrap();

    assert_eq!(parsed.name, "Coverage Skill");
    assert_eq!(parsed.description, "Parsed from disk");
    assert!(parsed.enabled);
    assert_eq!(parsed.tags, vec!["alpha".to_owned(), "beta".to_owned()]);
    assert_eq!(parsed.category.as_deref(), Some("tools"));
    assert!(parsed.instructions.contains("The real instructions."));
    assert!(!parsed.instructions.contains("ignored frontmatter"));
}

#[test]
fn a_skill_without_a_content_file_parses_with_empty_instructions() {
    let dir = tempfile::tempdir().unwrap();
    let skill = dir.path().join("bare");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("config.yaml"),
        "id: bare\nname: Bare Skill\ndescription: No content file\n",
    )
    .unwrap();

    let parsed = parse_skill_from_config(&skill.join("config.yaml"), &skill).unwrap();
    assert_eq!(parsed.name, "Bare Skill");
    assert!(parsed.instructions.is_empty());
}

#[test]
fn an_unreadable_or_invalid_skill_config_names_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("absent/config.yaml");
    let err = parse_skill_from_config(&missing, dir.path()).unwrap_err();
    assert!(format!("{err:#}").contains("config.yaml"));

    let broken = dir.path().join("config.yaml");
    std::fs::write(&broken, "name: [unterminated\n").unwrap();
    let err = parse_skill_from_config(&broken, dir.path()).unwrap_err();
    assert!(format!("{err:#}").contains("config.yaml"));
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

fn plugin_show(
    root: &Path,
    id: &str,
) -> anyhow::Result<systemprompt_extension::cli::CommandOutput> {
    let args = systemprompt_marketplace_entry::cli::plugins::show::ShowArgs {
        id: systemprompt_identifiers::PluginId::try_new(id)?,
    };
    systemprompt_marketplace_entry::cli::plugins::show::execute_with_path(&args, root)
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
