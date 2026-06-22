//! Behavioural tests for `merge_into` include-merge branches, driven through
//! the public `ConfigLoader::load_from_content` path.
//!
//! Each test writes an include file into a temp dir and references it from a
//! root config, then asserts the merged `ServicesConfig` reflects the
//! documented merge semantics (accumulate AI providers, root-priority for
//! `web`/`scheduler`, hard error on duplicate keys).

use systemprompt_loader::ConfigLoader;
use tempfile::TempDir;

const ROOT_HEAD: &str = r#"
includes:
  - include.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;

fn write_include(temp: &TempDir, body: &str) {
    std::fs::write(temp.path().join("include.yaml"), body).expect("write include");
}

#[test]
fn ai_providers_taken_from_include_when_root_empty() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        r#"
agents: {}
mcp_servers: {}
ai:
  default_provider: anthropic
  providers:
    anthropic: {}
"#,
    );

    let root = format!("{ROOT_HEAD}\nai:\n  default_provider: anthropic\n");
    let config = ConfigLoader::load_from_content(&root, &config_path).expect("should load");

    assert!(
        config.ai.providers.contains_key("anthropic"),
        "include's provider must populate the empty root provider map"
    );
    assert_eq!(config.ai.default_provider, "anthropic");
}

#[test]
fn ai_providers_accumulate_across_root_and_include() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        r#"
agents: {}
mcp_servers: {}
ai:
  default_provider: openai
  providers:
    openai: {}
"#,
    );

    let root = format!(
        "{ROOT_HEAD}\nai:\n  default_provider: anthropic\n  providers:\n    anthropic: {{}}\n"
    );
    let config = ConfigLoader::load_from_content(&root, &config_path).expect("should load");

    assert!(
        config.ai.providers.contains_key("anthropic"),
        "root provider retained"
    );
    assert!(
        config.ai.providers.contains_key("openai"),
        "include provider folded into existing root map"
    );
    assert_eq!(
        config.ai.providers.len(),
        2,
        "providers from both sides must accumulate"
    );
    assert_eq!(
        config.ai.default_provider, "anthropic",
        "root default_provider must not be overwritten when merging into a non-empty map"
    );
}

#[test]
fn scheduler_carried_from_include_when_root_absent() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        r#"
agents: {}
mcp_servers: {}
scheduler:
  enabled: false
  jobs: []
  bootstrap_jobs: []
  distributed_lock: false
"#,
    );

    let root = format!("{ROOT_HEAD}\nai:\n  default_provider: anthropic\n");
    let config = ConfigLoader::load_from_content(&root, &config_path).expect("should load");

    let scheduler = config.scheduler.expect("scheduler taken from include");
    assert!(
        !scheduler.enabled,
        "include's scheduler block must be carried onto the empty root"
    );
}

#[test]
fn external_agents_merge_from_include() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        r#"
agents: {}
mcp_servers: {}
external_agents:
  claude_desktop:
    id: claude_desktop
    display_name: Claude Desktop
    kind: desktop_app
    enabled: true
"#,
    );

    let root = format!("{ROOT_HEAD}\nai:\n  default_provider: anthropic\n");
    let config = ConfigLoader::load_from_content(&root, &config_path).expect("should load");

    let found = config
        .external_agents
        .iter()
        .any(|(id, _)| id.as_str() == "claude_desktop");
    assert!(found, "external agent from include must be merged in");
}

#[test]
fn duplicate_external_agent_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        r#"
agents: {}
mcp_servers: {}
external_agents:
  codex_cli:
    id: codex_cli
    display_name: Codex CLI (include)
    kind: cli_tool
    enabled: true
"#,
    );

    let root = format!(
        "{ROOT_HEAD}\nai:\n  default_provider: anthropic\nexternal_agents:\n  codex_cli:\n    id: codex_cli\n    display_name: Codex CLI (root)\n    kind: cli_tool\n    enabled: true\n"
    );
    let err = ConfigLoader::load_from_content(&root, &config_path)
        .expect_err("duplicate external agent must error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("codex_cli") || msg.to_lowercase().contains("external agent"),
        "expected duplicate external-agent error, got: {msg}"
    );
}

#[test]
fn duplicate_plugin_in_include_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let plugin_block = |name: &str| {
        format!(
            r#"
  dup-plugin:
    id: dup-plugin
    name: {name}
    description: dup
    version: 1.0.0
    enabled: true
    author:
      name: a
      email: a@b.com
    keywords: []
    license: MIT
    category: platform
    skills: {{source: instance}}
    agents: {{source: instance}}
    mcp_servers: {{include: []}}
    content_sources: {{}}
    scripts: []
"#
        )
    };

    write_include(
        &temp,
        &format!(
            "agents: {{}}\nmcp_servers: {{}}\nplugins:{}",
            plugin_block("Include Plugin")
        ),
    );

    let root = format!(
        "{ROOT_HEAD}\nai:\n  default_provider: anthropic\nplugins:{}",
        plugin_block("Root Plugin")
    );
    let err = ConfigLoader::load_from_content(&root, &config_path)
        .expect_err("duplicate plugin must error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("dup-plugin") || msg.to_lowercase().contains("plugin"),
        "expected duplicate plugin error, got: {msg}"
    );
}

#[test]
fn skills_merge_auto_discover_and_path_from_include() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        r#"
agents: {}
mcp_servers: {}
skills:
  enabled: true
  auto_discover: true
  skills_path: /some/skills/path
  skills:
    inc_skill:
      id: inc_skill
      name: Included Skill
      description: from include
      enabled: true
      tags: []
      assigned_agents: {include: []}
      mcp_servers: {include: []}
"#,
    );

    let root = format!(
        "{ROOT_HEAD}\nai:\n  default_provider: anthropic\nskills:\n  enabled: true\n  auto_discover: false\n  skills: {{}}\n"
    );
    let config = ConfigLoader::load_from_content(&root, &config_path).expect("should load");

    assert!(
        config.skills.auto_discover,
        "include's auto_discover: true must propagate onto the root"
    );
    assert_eq!(
        config.skills.skills_path.as_deref(),
        Some("/some/skills/path"),
        "include's skills_path must be adopted when root left it unset"
    );
    assert!(
        config.skills.skills.contains_key("inc_skill"),
        "skill from include must be merged in"
    );
}

#[test]
fn duplicate_skill_across_root_and_include_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let skill_block = |desc: &str| {
        format!(
            r#"
    clash_skill:
      id: clash_skill
      name: Clash Skill
      description: {desc}
      enabled: true
      tags: []
      assigned_agents: {{include: []}}
      mcp_servers: {{include: []}}
"#
        )
    };

    write_include(
        &temp,
        &format!(
            "agents: {{}}\nmcp_servers: {{}}\nskills:\n  enabled: true\n  auto_discover: false\n  skills:{}",
            skill_block("from include")
        ),
    );

    let root = format!(
        "{ROOT_HEAD}\nai:\n  default_provider: anthropic\nskills:\n  enabled: true\n  auto_discover: false\n  skills:{}",
        skill_block("from root")
    );
    let err = ConfigLoader::load_from_content(&root, &config_path)
        .expect_err("duplicate skill must error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("clash_skill") || msg.to_lowercase().contains("skill"),
        "expected duplicate skill error, got: {msg}"
    );
}

#[test]
fn include_cycle_is_rejected() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    std::fs::write(
        temp.path().join("a.yaml"),
        "agents: {}\nmcp_servers: {}\nincludes:\n  - b.yaml\n",
    )
    .expect("write a");
    std::fs::write(
        temp.path().join("b.yaml"),
        "agents: {}\nmcp_servers: {}\nincludes:\n  - a.yaml\n",
    )
    .expect("write b");

    let root = r#"
includes:
  - a.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(root, &config_path)
        .expect_err("include cycle must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.to_lowercase().contains("cycle") || msg.contains("->"),
        "expected include-cycle error, got: {msg}"
    );
}

#[test]
fn nested_include_resolves_transitively() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    std::fs::write(
        temp.path().join("level1.yaml"),
        "agents: {}\nmcp_servers: {}\nincludes:\n  - level2.yaml\n",
    )
    .expect("write level1");
    std::fs::write(
        temp.path().join("level2.yaml"),
        r#"
agents: {}
mcp_servers: {}
ai:
  default_provider: gemini
  providers:
    gemini: {}
"#,
    )
    .expect("write level2");

    let root = r#"
includes:
  - level1.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: gemini
"#;

    let config = ConfigLoader::load_from_content(root, &config_path).expect("should load");
    assert!(
        config.ai.providers.contains_key("gemini"),
        "transitively-included provider must reach the merged config"
    );
}
