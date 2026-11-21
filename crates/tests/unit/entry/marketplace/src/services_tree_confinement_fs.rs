//! Reads through the services tree stay inside it: a skill whose directory or
//! instruction file resolves outside the skills root, and a plugin script
//! whose source leaves the plugin directory or whose name is not one path
//! segment, are refused. Every enumeration of the services tree refuses a
//! directory holding an entry that resolves outside its root, and every file
//! a command reads from an enumerated entry is confined the same way.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;

use systemprompt_identifiers::{PluginId, SkillId};
use systemprompt_manifest::PluginConfigFile;
use systemprompt_marketplace_entry::cli::plugins::generate::marketplace;
use systemprompt_marketplace_entry::cli::plugins::generate::skills::generate_skills;
use systemprompt_marketplace_entry::cli::plugins::list::scan_plugins;
use systemprompt_marketplace_entry::cli::plugins::validate::validate_plugin;
use systemprompt_marketplace_entry::cli::plugins::collect_plugin_ids;
use systemprompt_marketplace_entry::cli::skills::list::{ListArgs, execute_with_path, show_skill_detail};
use systemprompt_marketplace_entry::cli::skills::types::parse_skill_from_config;

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
    include: []
  agents:
    source: explicit
    include: []
"#;

#[test]
fn skill_ids_that_traverse_are_refused_by_the_cli_parser() {
    for bad in ["../x", "a/b", ".."] {
        assert!(
            systemprompt_extension::cli::parsers::parse_skill_id(bad).is_err(),
            "parser accepted {bad:?}"
        );
        assert!(
            systemprompt_extension::cli::parsers::parse_plugin_id(bad).is_err(),
            "parser accepted {bad:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn skill_detail_refuses_a_skill_directory_linked_outside_the_tree() {
    let skills = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(
        outside.path().join("config.yaml"),
        "id: leak\nname: leak\ndescription: d\nenabled: true\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), skills.path().join("leak")).unwrap();

    let err = show_skill_detail(&SkillId::new("leak"), skills.path()).unwrap_err();
    assert!(err.to_string().contains("outside"), "{err}");
}

#[test]
fn skill_content_file_outside_the_skill_directory_is_refused() {
    let skills = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.md");
    fs::write(&secret, "do not read").unwrap();

    let skill_dir = skills.path().join("absolute");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("config.yaml"),
        format!(
            "id: absolute\nname: absolute\ndescription: d\nenabled: true\nfile: {}\n",
            secret.display()
        ),
    )
    .unwrap();

    let err = parse_skill_from_config(&skill_dir.join("config.yaml"), &skill_dir).unwrap_err();
    assert!(format!("{err:#}").contains("outside"), "{err:#}");
}

#[test]
fn copy_scripts_refuses_a_source_outside_the_plugin_directory() {
    let plugins = tempfile::tempdir().unwrap();
    fs::create_dir_all(plugins.path().join("demo")).unwrap();
    fs::write(plugins.path().join("loot.sh"), "echo loot").unwrap();

    let yaml = format!("{PLUGIN_YAML}  scripts:\n    - name: run.sh\n      source: ../loot.sh\n");
    let plugin = serde_yaml::from_str::<PluginConfigFile>(&yaml)
        .unwrap()
        .plugin;
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();

    let err = marketplace::copy_scripts(
        &plugin,
        plugins.path(),
        &PluginId::new("demo"),
        out.path(),
        &mut files,
    )
    .unwrap_err();
    assert!(format!("{err:#}").contains("outside"), "{err:#}");
    assert!(files.is_empty());
}

#[test]
fn copy_scripts_refuses_a_destination_name_that_is_not_one_segment() {
    let plugins = tempfile::tempdir().unwrap();
    fs::create_dir_all(plugins.path().join("demo/scripts")).unwrap();
    fs::write(plugins.path().join("demo/scripts/run.sh"), "echo hi").unwrap();

    let yaml = format!(
        "{PLUGIN_YAML}  scripts:\n    - name: ../../escaped.sh\n      source: scripts/run.sh\n"
    );
    let plugin = serde_yaml::from_str::<PluginConfigFile>(&yaml)
        .unwrap()
        .plugin;
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();

    assert!(
        marketplace::copy_scripts(
            &plugin,
            plugins.path(),
            &PluginId::new("demo"),
            &out.path().join("nested"),
            &mut files,
        )
        .is_err()
    );
    assert!(!out.path().join("escaped.sh").exists());
}

#[cfg(unix)]
fn linked_skill_tree() -> (tempfile::TempDir, tempfile::TempDir) {
    let skills = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(
        outside.path().join("config.yaml"),
        "id: leak\nname: leak\ndescription: d\nenabled: true\n",
    )
    .unwrap();
    fs::write(outside.path().join("index.md"), "outside body").unwrap();
    std::os::unix::fs::symlink(outside.path(), skills.path().join("leak")).unwrap();
    (skills, outside)
}

#[cfg(unix)]
#[test]
fn skills_list_refuses_a_tree_holding_a_skill_linked_outside_it() {
    let (skills, _outside) = linked_skill_tree();
    let args = ListArgs {
        name: None,
        enabled: false,
        disabled: false,
    };
    let err = execute_with_path(args, skills.path()).unwrap_err();
    assert!(format!("{err:#}").contains("outside"), "{err:#}");
}

#[cfg(unix)]
#[test]
fn plugin_enumerations_refuse_a_plugin_directory_linked_outside_the_tree() {
    let plugins = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("config.yaml"), PLUGIN_YAML).unwrap();
    std::os::unix::fs::symlink(outside.path(), plugins.path().join("demo")).unwrap();

    let listed = scan_plugins(plugins.path()).unwrap_err();
    assert!(format!("{listed:#}").contains("outside"), "{listed:#}");
    let ids = collect_plugin_ids(plugins.path()).unwrap_err();
    assert!(format!("{ids:#}").contains("outside"), "{ids:#}");
}

#[cfg(unix)]
#[test]
fn generate_refuses_an_enumerated_skill_linked_outside_the_tree() {
    let (skills, _outside) = linked_skill_tree();
    let yaml = PLUGIN_YAML.replace(
        "  skills:\n    source: explicit\n    include: []\n",
        "  skills:\n    source: instance\n",
    );
    let plugin = serde_yaml::from_str::<PluginConfigFile>(&yaml).unwrap().plugin;
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();

    let err = generate_skills(&plugin, skills.path(), out.path(), &mut files).unwrap_err();
    assert!(format!("{err:#}").contains("outside"), "{err:#}");
    assert!(files.is_empty());
}

#[cfg(unix)]
#[test]
fn generate_refuses_a_skill_body_file_linked_outside_the_tree() {
    let skills = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.md"), "outside body").unwrap();
    let skill = skills.path().join("inner");
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("config.yaml"), "name: inner\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret.md"), skill.join("index.md")).unwrap();
    let yaml = PLUGIN_YAML.replace("    include: []\n  agents", "    include: [inner]\n  agents");
    let plugin = serde_yaml::from_str::<PluginConfigFile>(&yaml).unwrap().plugin;
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();

    let err = generate_skills(&plugin, skills.path(), out.path(), &mut files).unwrap_err();
    assert!(format!("{err:#}").contains("outside"), "{err:#}");
    assert!(!out.path().join("skills/inner/SKILL.md").exists());
}

#[cfg(unix)]
#[test]
fn validate_reports_a_skill_linked_outside_the_tree() {
    let (skills, _outside) = linked_skill_tree();
    let plugins = tempfile::tempdir().unwrap();
    fs::create_dir_all(plugins.path().join("demo")).unwrap();
    let yaml = PLUGIN_YAML.replace("    include: []\n  agents", "    include: [leak]\n  agents");
    fs::write(plugins.path().join("demo/config.yaml"), yaml).unwrap();

    let report = validate_plugin(&PluginId::new("demo"), plugins.path(), skills.path());
    assert!(!report.valid);
    assert!(
        report.errors.iter().any(|e| e.contains("outside")),
        "{:?}",
        report.errors
    );
}
