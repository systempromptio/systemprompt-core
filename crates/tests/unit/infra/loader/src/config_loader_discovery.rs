use systemprompt_loader::ConfigLoader;
use tempfile::TempDir;

fn base_config() -> &'static str {
    r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#
}

fn write_skill_config(dir: &std::path::Path, id: &str, name: &str) {
    std::fs::create_dir_all(dir).expect("create skill dir");
    let content = format!(
        r#"
id: {id}
name: {name}
description: Auto-discovered skill
enabled: true
tags: [test]
"#
    );
    std::fs::write(dir.join("config.yaml"), content).expect("write skill config");
}

fn write_plugin_config(dir: &std::path::Path, id: &str, name: &str) {
    std::fs::create_dir_all(dir).expect("create plugin dir");
    let content = format!(
        r#"
plugin:
  id: {id}
  name: {name}
  description: Auto-discovered plugin
  version: 1.0.0
  enabled: true
  author:
    name: fixture
    email: fixture@example.com
  keywords: []
  license: MIT
  category: platform
  skills:
    source: instance
  agents:
    source: instance
  mcp_servers:
    include: []
  content_sources: {{}}
  scripts: []
"#
    );
    std::fs::write(dir.join("config.yaml"), content).expect("write plugin config");
}

fn write_marketplace_config(dir: &std::path::Path, id: &str, name: &str) {
    std::fs::create_dir_all(dir).expect("create marketplace dir");
    let content = format!(
        r#"
marketplace:
  id: {id}
  name: {name}
  description: Auto-discovered marketplace
  version: 1.0.0
  enabled: true
  author:
    name: fixture
    email: fixture@example.com
  keywords: []
  license: MIT
"#
    );
    std::fs::write(dir.join("config.yaml"), content).expect("write marketplace config");
}

#[test]
fn discover_skills_from_disk() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    let services_dir = temp.path();
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let skills_dir = services_dir.join("skills");
    write_skill_config(
        &skills_dir.join("auto-skill-one"),
        "auto-skill-one",
        "Auto Skill One",
    );
    write_skill_config(
        &skills_dir.join("auto-skill-two"),
        "auto-skill-two",
        "Auto Skill Two",
    );

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    assert!(
        config.skills.skills.contains_key("auto-skill-one"),
        "auto-skill-one should be discovered"
    );
    assert!(
        config.skills.skills.contains_key("auto-skill-two"),
        "auto-skill-two should be discovered"
    );
}

#[test]
fn discover_skills_skips_non_dirs() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    let services_dir = temp.path();
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let skills_dir = services_dir.join("skills");
    std::fs::create_dir_all(&skills_dir).expect("create skills dir");
    std::fs::write(skills_dir.join("not-a-dir.txt"), "just a file").expect("write file");

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    assert!(config.skills.skills.is_empty());
}

#[test]
fn discover_skills_skips_dirs_without_config() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    let services_dir = temp.path();
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let skills_dir = services_dir.join("skills");
    let no_config_dir = skills_dir.join("incomplete-skill");
    std::fs::create_dir_all(&no_config_dir).expect("create dir without config");
    std::fs::write(no_config_dir.join("index.md"), "content").expect("write index");

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    assert!(config.skills.skills.is_empty());
}

#[test]
fn discover_skills_does_not_override_explicit() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    let services_dir = temp.path();
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let explicit_skill_yaml = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
skills:
  enabled: true
  auto_discover: false
  skills:
    my-skill:
      id: my-skill
      name: Explicit Skill
      description: Explicitly defined skill
      enabled: true
      tags: []
      instructions: "Explicit instructions"
      assigned_agents: {include: []}
      mcp_servers: {include: []}
"#;
    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, explicit_skill_yaml).expect("write config");

    let skills_dir = services_dir.join("skills");
    write_skill_config(&skills_dir.join("my-skill"), "my-skill", "Disk Skill Name");

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    let skill = config.skills.skills.get("my-skill").expect("skill present");
    assert_eq!(
        skill.name, "Explicit Skill",
        "explicit definition must win over disk"
    );
}

#[test]
fn discover_plugins_from_disk() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    let services_dir = temp.path();
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let plugins_dir = services_dir.join("plugins");
    write_plugin_config(&plugins_dir.join("my-plugin"), "my-plugin", "My Plugin");

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    assert!(
        config.plugins.contains_key("my-plugin"),
        "plugin must be discovered from disk"
    );
}

#[test]
fn discover_plugins_skips_non_dirs() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");
    std::fs::write(plugins_dir.join("not-a-dir.txt"), "data").expect("write file");

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    assert!(config.plugins.is_empty());
}

#[test]
fn discover_plugins_skips_dirs_without_config() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let plugins_dir = temp.path().join("plugins");
    let no_config = plugins_dir.join("incomplete");
    std::fs::create_dir_all(&no_config).expect("create dir without config");
    std::fs::write(no_config.join("README.md"), "no config").expect("write readme");

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    assert!(config.plugins.is_empty());
}

#[test]
fn discover_plugins_does_not_override_explicit() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let explicit_yaml = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
plugins:
  my-plugin:
    id: my-plugin
    name: Explicit Plugin
    description: Explicitly declared
    version: 2.0.0
    enabled: true
    author:
      name: author
      email: a@b.com
    keywords: []
    license: MIT
    category: platform
    skills: {source: instance}
    agents: {source: instance}
    mcp_servers: {include: []}
    content_sources: {}
    scripts: []
"#;
    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, explicit_yaml).expect("write config");

    let plugins_dir = temp.path().join("plugins");
    write_plugin_config(
        &plugins_dir.join("my-plugin"),
        "my-plugin",
        "Disk Plugin Name",
    );

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    let plugin = config.plugins.get("my-plugin").expect("plugin present");
    assert_eq!(
        plugin.name, "Explicit Plugin",
        "explicit plugin must win over disk"
    );
}

#[test]
fn discover_marketplaces_from_disk() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    let services_dir = temp.path();
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let mkt_dir = services_dir.join("marketplaces");
    write_marketplace_config(&mkt_dir.join("my-market"), "my-market", "My Market");

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    let found = config
        .marketplaces
        .iter()
        .any(|(k, _)| k.as_str() == "my-market");
    assert!(found, "marketplace must be discovered from disk");
}

#[test]
fn discover_marketplaces_skips_non_dirs() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let mkt_dir = temp.path().join("marketplaces");
    std::fs::create_dir_all(&mkt_dir).expect("create marketplaces dir");
    std::fs::write(mkt_dir.join("stray.txt"), "data").expect("write stray");

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    assert!(config.marketplaces.is_empty());
}

#[test]
fn discover_marketplaces_skips_dirs_without_config() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let mkt_dir = temp.path().join("marketplaces");
    let no_cfg = mkt_dir.join("no-config");
    std::fs::create_dir_all(&no_cfg).expect("create dir");
    std::fs::write(no_cfg.join("README.md"), "empty").expect("write readme");

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    assert!(config.marketplaces.is_empty());
}

#[test]
fn duplicate_marketplace_via_disk_discovery_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    let services_dir = temp.path();
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let mkt_dir = services_dir.join("marketplaces");
    write_marketplace_config(&mkt_dir.join("dup-a"), "dup-market", "Dup A");
    write_marketplace_config(&mkt_dir.join("dup-b"), "dup-market", "Dup B");

    let result = ConfigLoader::load_from_path(&config_path);
    assert!(
        result.is_err(),
        "duplicate marketplace IDs from disk must error"
    );
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("dup-market") || msg.contains("duplicate marketplace"),
        "expected duplicate marketplace error, got: {msg}"
    );
}

#[test]
fn discover_multiple_plugins_and_skills() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    let services_dir = temp.path();
    std::fs::create_dir_all(&config_dir).expect("create config dir");

    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let skills_dir = services_dir.join("skills");
    let plugins_dir = services_dir.join("plugins");

    for i in 1..=3 {
        write_skill_config(
            &skills_dir.join(format!("skill-{i}")),
            &format!("skill-{i}"),
            &format!("Skill {i}"),
        );
        write_plugin_config(
            &plugins_dir.join(format!("plugin-{i}")),
            &format!("plugin-{i}"),
            &format!("Plugin {i}"),
        );
    }

    let config = ConfigLoader::load_from_path(&config_path).expect("should load");
    assert_eq!(config.skills.skills.len(), 3);
    assert_eq!(config.plugins.len(), 3);
}

#[test]
fn discover_skills_malformed_yaml_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let skill_dir = temp.path().join("skills").join("broken-skill");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("config.yaml"), "id: : : not valid: yaml")
        .expect("write malformed skill config");

    let result = ConfigLoader::load_from_path(&config_path);
    let err = result.expect_err("malformed skill config.yaml must surface a parse error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("broken-skill") || msg.to_lowercase().contains("yaml"),
        "expected a YAML parse error naming the offending file, got: {msg}"
    );
}

#[test]
fn discover_plugins_malformed_yaml_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let plugin_dir = temp.path().join("plugins").join("broken-plugin");
    std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");
    std::fs::write(plugin_dir.join("config.yaml"), "plugin: : : bad: yaml")
        .expect("write malformed plugin config");

    let result = ConfigLoader::load_from_path(&config_path);
    let err = result.expect_err("malformed plugin config.yaml must surface a parse error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("broken-plugin") || msg.to_lowercase().contains("yaml"),
        "expected a YAML parse error naming the offending file, got: {msg}"
    );
}

#[test]
fn discover_marketplaces_malformed_yaml_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    let config_path = config_dir.join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let mkt_dir = temp.path().join("marketplaces").join("broken-market");
    std::fs::create_dir_all(&mkt_dir).expect("create marketplace dir");
    std::fs::write(mkt_dir.join("config.yaml"), "marketplace: : : bad: yaml")
        .expect("write malformed marketplace config");

    let result = ConfigLoader::load_from_path(&config_path);
    let err = result.expect_err("malformed marketplace config.yaml must surface a parse error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("broken-market") || msg.to_lowercase().contains("yaml"),
        "expected a YAML parse error naming the offending file, got: {msg}"
    );
}

#[test]
fn discover_skips_missing_parent_no_panic() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let config = ConfigLoader::load_from_path(&config_path)
        .expect("should load even with no parent services dir structure");
    assert!(config.skills.skills.is_empty());
    assert!(config.plugins.is_empty());
    assert!(config.marketplaces.is_empty());
}
