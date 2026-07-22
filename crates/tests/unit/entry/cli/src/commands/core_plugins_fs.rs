//! Filesystem-driven tests for `core plugins` validate and generate helpers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::core::plugins::generate::agents::generate_agents;
use systemprompt_cli::core::plugins::generate::mcp::generate_mcp_json;
use systemprompt_cli::core::plugins::generate::skills::generate_skills;
use systemprompt_cli::core::plugins::generate::{
    PluginGenerateContext, generate_plugin, marketplace,
};
use systemprompt_cli::core::plugins::{generate, validate};
use systemprompt_models::services::ServicesConfig;
use systemprompt_models::{PluginConfig, PluginConfigFile};

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

fn parse_plugin(yaml: &str) -> PluginConfig {
    let file: PluginConfigFile = serde_yaml::from_str(yaml).unwrap();
    file.plugin
}

fn write_plugin_dir(root: &Path, id: &str, yaml: &str) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.yaml"), yaml).unwrap();
}

#[test]
fn collect_plugin_ids_handles_missing_and_mixed_dirs() {
    let missing = Path::new("/nonexistent/plugins-dir");
    assert!(validate::collect_plugin_ids(missing).unwrap().is_empty());
    assert!(generate::collect_plugin_ids(missing).unwrap().is_empty());

    let tmp = tempfile::tempdir().unwrap();
    write_plugin_dir(tmp.path(), "zeta", PLUGIN_YAML);
    write_plugin_dir(tmp.path(), "alpha", PLUGIN_YAML);
    fs::create_dir_all(tmp.path().join("no-config")).unwrap();
    fs::write(tmp.path().join("stray.txt"), "x").unwrap();

    let ids = validate::collect_plugin_ids(tmp.path()).unwrap();
    assert_eq!(ids, vec!["alpha".to_owned(), "zeta".to_owned()]);
    assert_eq!(generate::collect_plugin_ids(tmp.path()).unwrap(), ids);
}

#[test]
fn validate_plugin_reports_missing_config() {
    let tmp = tempfile::tempdir().unwrap();
    let out = validate::validate_plugin("ghost", tmp.path(), tmp.path());
    assert!(!out.valid);
    assert!(out.errors[0].contains("Failed to read config.yaml"));
}

#[test]
fn validate_plugin_reports_parse_error() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin_dir(tmp.path(), "bad", "plugin: [not, a, mapping");
    let out = validate::validate_plugin("bad", tmp.path(), tmp.path());
    assert!(!out.valid);
    assert!(out.errors[0].contains("Failed to parse config.yaml"));
}

#[test]
fn validate_plugin_flags_id_mismatch_and_missing_refs() {
    let tmp = tempfile::tempdir().unwrap();
    let yaml = PLUGIN_YAML.replace("id: demo", "id: other");
    write_plugin_dir(tmp.path(), "demo", &yaml);
    let skills = tempfile::tempdir().unwrap();

    let out = validate::validate_plugin("demo", tmp.path(), skills.path());
    assert!(!out.valid);
    assert!(
        out.errors
            .iter()
            .any(|e| e.contains("Referenced skill 'alpha_skill' not found"))
    );
    assert!(
        out.warnings
            .iter()
            .any(|w| w.contains("does not match directory name"))
    );
}

#[test]
fn validate_plugin_checks_scripts_and_passes_when_complete() {
    let tmp = tempfile::tempdir().unwrap();
    let yaml =
        format!("{PLUGIN_YAML}  scripts:\n    - name: run.sh\n      source: scripts/run.sh\n");
    write_plugin_dir(tmp.path(), "demo", &yaml);
    let skills = tempfile::tempdir().unwrap();
    fs::create_dir_all(skills.path().join("alpha_skill")).unwrap();

    let out = validate::validate_plugin("demo", tmp.path(), skills.path());
    assert!(!out.valid);
    assert!(
        out.errors
            .iter()
            .any(|e| e.contains("Script 'run.sh' not found"))
    );

    fs::create_dir_all(tmp.path().join("demo/scripts")).unwrap();
    fs::write(tmp.path().join("demo/scripts/run.sh"), "#!/bin/sh\n").unwrap();
    let out = validate::validate_plugin("demo", tmp.path(), skills.path());
    assert!(out.valid, "errors: {:?}", out.errors);
}

#[test]
fn generate_skills_explicit_uses_config_and_index() {
    let skills = tempfile::tempdir().unwrap();
    let skill_dir = skills.path().join("alpha_skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("config.yaml"),
        "name: Alpha\ndescription: \"Does \\\"things\\\"\"\n",
    )
    .unwrap();
    fs::write(skill_dir.join("index.md"), "---\ntitle: x\n---\nSkill body").unwrap();

    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    generate_skills(
        &parse_plugin(PLUGIN_YAML),
        skills.path(),
        out.path(),
        &mut files,
    )
    .unwrap();

    assert_eq!(files.len(), 1);
    let content = fs::read_to_string(out.path().join("skills/alpha-skill/SKILL.md")).unwrap();
    assert!(content.contains("name: \"Alpha\""));
    assert!(content.contains("Skill body"));
    assert!(!content.contains("title: x"));
}

#[test]
fn generate_skills_instance_scans_and_filters() {
    let skills = tempfile::tempdir().unwrap();
    for (id, enabled) in [("keep_me", true), ("drop_me", false), ("excluded", true)] {
        let dir = skills.path().join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("config.yaml"),
            format!("name: {id}\nenabled: {enabled}\n"),
        )
        .unwrap();
    }

    let yaml = r#"
plugin:
  id: demo
  name: Demo
  description: d
  version: 1.0.0
  author: { name: T, email: t@example.com }
  keywords: []
  license: MIT
  category: tools
  skills:
    source: instance
    filter: enabled
    exclude: [excluded]
  agents:
    source: explicit
"#;
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    generate_skills(&parse_plugin(yaml), skills.path(), out.path(), &mut files).unwrap();

    assert_eq!(files.len(), 1);
    assert!(out.path().join("skills/keep-me/SKILL.md").exists());
    assert!(!out.path().join("skills/drop-me/SKILL.md").exists());
    assert!(!out.path().join("skills/excluded/SKILL.md").exists());
}

#[test]
fn generate_skills_placeholder_body_without_sources() {
    let skills = tempfile::tempdir().unwrap();
    fs::create_dir_all(skills.path().join("alpha_skill")).unwrap();

    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    generate_skills(
        &parse_plugin(PLUGIN_YAML),
        skills.path(),
        out.path(),
        &mut files,
    )
    .unwrap();
    let content = fs::read_to_string(out.path().join("skills/alpha-skill/SKILL.md")).unwrap();
    assert!(content.contains("systemprompt core skills show alpha_skill"));
}

#[test]
fn generate_agents_falls_back_without_services_config() {
    let services = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    generate_agents(
        &parse_plugin(PLUGIN_YAML),
        services.path(),
        out.path(),
        &mut files,
    )
    .unwrap();

    assert_eq!(files.len(), 1);
    let content = fs::read_to_string(out.path().join("agents/helper.md")).unwrap();
    assert!(content.contains("name: helper"));
    assert!(content.contains("You are the helper agent."));
}

#[test]
fn generate_agents_reads_agent_yaml_definitions() {
    let services = tempfile::tempdir().unwrap();
    let agents_dir = services.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("helper.yaml"),
        r#"
agents:
  helper:
    card:
      description: "Helps with \"stuff\""
    metadata:
      systemPrompt: "You help."
"#,
    )
    .unwrap();
    fs::write(agents_dir.join("broken.yaml"), "agents: [oops").unwrap();

    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    generate_agents(
        &parse_plugin(PLUGIN_YAML),
        services.path(),
        out.path(),
        &mut files,
    )
    .unwrap();

    let content = fs::read_to_string(out.path().join("agents/helper.md")).unwrap();
    assert!(content.contains("Helps with"));
    assert!(content.contains("You help."));
}

#[test]
fn generate_agents_instance_lists_from_services_config() {
    let services = tempfile::tempdir().unwrap();
    fs::create_dir_all(services.path().join("config")).unwrap();
    fs::write(
        services.path().join("config/config.yaml"),
        "agents:\n  one: {}\n  two: {}\n",
    )
    .unwrap();

    let yaml = PLUGIN_YAML.replace(
        "  agents:\n    source: explicit\n    include: [helper]",
        "  agents:\n    source: instance\n    exclude: [two]",
    );
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    generate_agents(
        &parse_plugin(&yaml),
        services.path(),
        out.path(),
        &mut files,
    )
    .unwrap();

    assert!(out.path().join("agents/one.md").exists());
    assert!(!out.path().join("agents/two.md").exists());
}

#[test]
fn generate_mcp_json_skips_when_no_servers() {
    let services = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    generate_mcp_json(
        &parse_plugin(PLUGIN_YAML),
        services.path(),
        out.path(),
        &mut files,
    )
    .unwrap();
    assert!(files.is_empty());
    assert!(!out.path().join(".mcp.json").exists());
}

#[test]
fn generate_mcp_json_resolves_ports() {
    let services = tempfile::tempdir().unwrap();
    fs::create_dir_all(services.path().join("config")).unwrap();
    fs::write(
        services.path().join("config/config.yaml"),
        "mcp_servers:\n  tools:\n    port: 5111\n",
    )
    .unwrap();

    let yaml =
        format!("{PLUGIN_YAML}  mcp_servers:\n    source: explicit\n    include: [tools, other]\n");
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    generate_mcp_json(
        &parse_plugin(&yaml),
        services.path(),
        out.path(),
        &mut files,
    )
    .unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out.path().join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(
        json["mcpServers"]["tools"]["url"],
        "http://localhost:5111/api/v1/mcp/tools/mcp"
    );
    assert_eq!(
        json["mcpServers"]["other"]["url"],
        "http://localhost:5000/api/v1/mcp/other/mcp"
    );
}

#[test]
fn generate_plugin_json_writes_manifest() {
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    marketplace::generate_plugin_json(&parse_plugin(PLUGIN_YAML), out.path(), &mut files).unwrap();

    let json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(out.path().join(".claude-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(json["name"], "demo");
    assert_eq!(json["version"], "1.0.0");
    assert_eq!(json["author"]["name"], "Tester");
}

#[test]
fn copy_scripts_copies_existing_sources_only() {
    let plugins = tempfile::tempdir().unwrap();
    write_plugin_dir(plugins.path(), "demo", PLUGIN_YAML);
    fs::create_dir_all(plugins.path().join("demo/scripts")).unwrap();
    fs::write(plugins.path().join("demo/scripts/run.sh"), "echo hi").unwrap();

    let yaml = format!(
        "{PLUGIN_YAML}  scripts:\n    - name: run.sh\n      source: scripts/run.sh\n    - name: \
         missing.sh\n      source: scripts/missing.sh\n"
    );
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    marketplace::copy_scripts(
        &parse_plugin(&yaml),
        plugins.path(),
        "demo",
        out.path(),
        &mut files,
    )
    .unwrap();

    assert_eq!(files.len(), 1);
    assert!(out.path().join("scripts/run.sh").exists());
    assert!(!out.path().join("scripts/missing.sh").exists());
}

#[test]
fn copy_scripts_noop_without_scripts() {
    let plugins = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    marketplace::copy_scripts(
        &parse_plugin(PLUGIN_YAML),
        plugins.path(),
        "demo",
        out.path(),
        &mut files,
    )
    .unwrap();
    assert!(files.is_empty());
}

#[test]
fn render_marketplace_includes_declared_plugins() {
    let services: ServicesConfig = serde_yaml::from_str(&format!(
        r#"
plugins:
  demo:
{}
marketplaces:
  main:
    id: main
    name: Main
    description: The main marketplace
    version: 2.0.0
    author: {{ name: Owner, email: o@example.com }}
    license: MIT
    plugins:
      source: explicit
      include: [demo, ghost]
"#,
        PLUGIN_YAML
            .lines()
            .skip(2)
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    ))
    .unwrap();

    let marketplace_cfg = services.marketplaces.values().next().unwrap();
    let json = marketplace::render_marketplace("main", marketplace_cfg, &services);
    assert_eq!(json["name"], "main");
    assert_eq!(json["owner"]["name"], "Owner");
    assert_eq!(json["metadata"]["version"], "2.0.0");
    let plugins = json["plugins"].as_array().unwrap();
    assert_eq!(plugins.len(), 2);
    let demo = plugins.iter().find(|p| p["name"] == "demo").unwrap();
    assert_eq!(demo["source"], "./storage/files/plugins/demo");
    assert_eq!(demo["version"], "1.0.0");
    let ghost = plugins.iter().find(|p| p["name"] == "ghost").unwrap();
    assert_eq!(ghost["version"], "");
}

#[test]
fn generate_plugin_materialises_full_output() {
    let plugins = tempfile::tempdir().unwrap();
    write_plugin_dir(plugins.path(), "demo", PLUGIN_YAML);
    let skills = tempfile::tempdir().unwrap();
    fs::create_dir_all(skills.path().join("alpha_skill")).unwrap();
    let services = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let out_str = out.path().to_string_lossy().to_string();

    let ctx = PluginGenerateContext {
        plugins_path: plugins.path(),
        skills_path: skills.path(),
        services_path: services.path(),
        output_dir_override: Some(&out_str),
    };
    let result = generate_plugin("demo", &ctx).unwrap();

    assert_eq!(result.plugin_id.as_str(), "demo");
    assert!(result.files_generated.len() >= 3);
    assert!(out.path().join(".claude-plugin/plugin.json").exists());
    assert!(out.path().join("skills/alpha-skill/SKILL.md").exists());
    assert!(out.path().join("agents/helper.md").exists());
}

#[test]
fn generate_plugin_errors_on_missing_config() {
    let plugins = tempfile::tempdir().unwrap();
    let skills = tempfile::tempdir().unwrap();
    let services = tempfile::tempdir().unwrap();
    let ctx = PluginGenerateContext {
        plugins_path: plugins.path(),
        skills_path: skills.path(),
        services_path: services.path(),
        output_dir_override: None,
    };
    let err = generate_plugin("ghost", &ctx).unwrap_err();
    assert!(err.to_string().contains("Failed to read"));
}

#[test]
fn extract_install_command_parses_github_repo_paths() {
    use systemprompt_cli::core::plugins::generate::extract_install_command;

    assert_eq!(extract_install_command(None), None);
    assert_eq!(
        extract_install_command(Some("https://github.com/acme/plugins")).as_deref(),
        Some("/plugin marketplace add acme/plugins")
    );
    assert_eq!(
        extract_install_command(Some("https://github.com/acme/plugins.git/")).as_deref(),
        Some("/plugin marketplace add acme/plugins")
    );
    assert_eq!(
        extract_install_command(Some("https://github.com/acme")),
        None
    );
}
