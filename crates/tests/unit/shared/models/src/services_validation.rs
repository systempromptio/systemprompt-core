use systemprompt_models::services::ServicesConfig;

fn agent_yaml(name: &str, port: u16, default: bool) -> String {
    format!(
        r"
  {name}:
    name: {name}
    port: {port}
    endpoint: /a2a
    enabled: true
    default: {default}
    card:
      protocolVersion: '1.0'
      displayName: Agent
      description: Test agent
      version: 1.0.0
    metadata: {{}}
"
    )
}

fn mcp_yaml(name: &str, port: u16, server_type: &str) -> String {
    let spawn = if server_type == "internal" {
        format!("    binary: bin\n    port: {port}\n")
    } else {
        "    endpoint: https://remote.example.com/mcp\n".to_owned()
    };
    format!(
        r"
  {name}:
    server_type: {server_type}
{spawn}    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
"
    )
}

fn plugin_yaml(name: &str, governance: bool, agents_ref: &str, mcp_ref: &str) -> String {
    format!(
        r"
  {name}:
    id: {name}
    name: {name}
    description: Test plugin
    version: 1.0.0
    enabled: true
    author:
      name: Ed
      email: ed@example.com
    keywords: []
    license: MIT
    category: tools
    skills: {{}}
    agents: {agents_ref}
    mcp_servers: {mcp_ref}
    hooks:
      governance: {governance}
"
    )
}

fn parse(yaml: &str) -> ServicesConfig {
    serde_yaml::from_str(yaml).unwrap()
}

#[test]
fn well_formed_config_validates() {
    let yaml = format!(
        "agents:{}mcp_servers:{}",
        agent_yaml("agent_one", 9001, true),
        mcp_yaml("tools", 5001, "internal")
    );
    assert!(parse(&yaml).validate().is_ok());
}

#[test]
fn duplicate_agent_ports_are_a_conflict() {
    let yaml = format!(
        "agents:{}{}",
        agent_yaml("agent_one", 9001, false),
        agent_yaml("agent_two", 9001, false)
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("Port conflict"));
}

#[test]
fn agent_and_internal_mcp_sharing_a_port_conflicts_only_in_range() {
    let yaml = format!(
        "agents:{}mcp_servers:{}",
        agent_yaml("agent_one", 9001, false),
        mcp_yaml("tools", 9001, "internal")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("Port conflict"));
}

#[test]
fn external_mcp_servers_bind_no_port_and_are_exempt_from_port_rules() {
    let yaml = format!(
        "agents:{}mcp_servers:{}",
        agent_yaml("agent_one", 9001, false),
        mcp_yaml("remote", 0, "external")
    );
    let config = parse(&yaml);
    assert_eq!(config.mcp_servers["remote"].port, None);
    assert!(config.validate().is_ok());
}

#[test]
fn agent_port_outside_default_range_is_rejected() {
    let yaml = format!("agents:{}", agent_yaml("agent_one", 8080, false));
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("outside allowed range 9000-9999"));
}

#[test]
fn internal_mcp_port_outside_default_range_is_rejected() {
    let yaml = format!("mcp_servers:{}", mcp_yaml("tools", 9500, "internal"));
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("outside allowed range 5000-5999"));
}

#[test]
fn custom_port_range_overrides_default() {
    let yaml = format!(
        "settings:\n  agent_port_range: [8000, 8100]\nagents:{}",
        agent_yaml("agent_one", 8080, false)
    );
    assert!(parse(&yaml).validate().is_ok());
}

#[test]
fn multiple_default_agents_are_rejected() {
    let yaml = format!(
        "agents:{}{}",
        agent_yaml("agent_one", 9001, true),
        agent_yaml("agent_two", 9002, true)
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(
        err.to_string()
            .contains("Multiple agents marked as default")
    );
    assert!(err.to_string().contains("agent_one"));
    assert!(err.to_string().contains("agent_two"));
}

#[test]
fn plugin_referencing_unknown_mcp_server_is_rejected() {
    let yaml = format!(
        "plugins:{}",
        plugin_yaml("plug", false, "{}", "{ include: [ghost] }")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("unknown mcp_server 'ghost'"));
}

#[test]
fn plugin_referencing_unknown_agent_is_rejected() {
    let yaml = format!(
        "plugins:{}",
        plugin_yaml("plug", false, "{ include: [ghost] }", "{}")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("unknown agent 'ghost'"));
}

#[test]
fn two_governance_hook_owners_are_rejected() {
    let yaml = format!(
        "plugins:{}{}",
        plugin_yaml("plug-a", true, "{}", "{}"),
        plugin_yaml("plug-b", true, "{}", "{}")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("hooks.governance"));
    assert!(err.to_string().contains("plug-a"));
    assert!(err.to_string().contains("plug-b"));
}

#[test]
fn disabled_plugin_does_not_count_as_governance_owner() {
    let enabled = plugin_yaml("plug-a", true, "{}", "{}");
    let disabled =
        plugin_yaml("plug-b", true, "{}", "{}").replace("enabled: true", "enabled: false");
    let yaml = format!("plugins:{enabled}{disabled}");
    assert!(parse(&yaml).validate().is_ok());
}

fn marketplace_yaml(id: &str, extra_refs: &str) -> String {
    format!(
        r"
  {id}:
    id: {id}
    name: Market
    description: Test marketplace
    version: 1.0.0
    author:
      name: Ed
      email: ed@example.com
    license: MIT
{extra_refs}"
    )
}

#[test]
fn marketplace_referencing_unknown_plugin_is_rejected() {
    let yaml = format!(
        "marketplaces:{}",
        marketplace_yaml("market", "    plugins:\n      include: [ghost]\n")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("unknown plugin 'ghost'"));
}

#[test]
fn two_marketplaces_need_no_default_selector() {
    let yaml = format!(
        "marketplaces:{}{}",
        marketplace_yaml("market-a", ""),
        marketplace_yaml("market-b", "")
    );
    parse(&yaml)
        .validate()
        .expect("several enabled marketplaces union into one manifest");
}

#[test]
fn default_marketplace_selector_must_match_a_configured_marketplace() {
    let yaml = format!(
        "settings:\n  default_marketplace_id: ghost\nmarketplaces:{}",
        marketplace_yaml("market-a", "")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("does not match any configured"));
}

#[test]
fn matching_default_marketplace_selector_passes() {
    let yaml = format!(
        "settings:\n  default_marketplace_id: market-a\nmarketplaces:{}{}",
        marketplace_yaml("market-a", ""),
        marketplace_yaml("market-b", "")
    );
    assert!(parse(&yaml).validate().is_ok());
}

fn skill_yaml(id: &str) -> String {
    format!(
        r"
    {id}:
      id: {id}
      name: Skill
      description: Test skill
      enabled: true
"
    )
}

#[test]
fn plugin_referencing_unknown_skill_is_rejected() {
    let yaml = format!(
        "plugins:{}",
        plugin_yaml("demo-plugin", false, "{}", "{}")
            .replace("skills: {}", "skills:\n      include: [ghost_skill]")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(
        err.to_string().contains("unknown skill 'ghost_skill'"),
        "{err}"
    );
}

#[test]
fn plugin_referencing_known_skill_passes() {
    let yaml = format!(
        "skills:\n  skills:{}\nplugins:{}",
        skill_yaml("real_skill"),
        plugin_yaml("demo-plugin", false, "{}", "{}")
            .replace("skills: {}", "skills:\n      include: [real_skill]")
    );
    assert!(parse(&yaml).validate().is_ok());
}

#[test]
fn hyphenated_skill_id_is_rejected() {
    let yaml = format!("skills:\n  skills:{}", skill_yaml("bad-skill"));
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("snake_case"), "{err}");
}

#[test]
fn snake_skill_id_passes() {
    let yaml = format!("skills:\n  skills:{}", skill_yaml("good_skill"));
    assert!(parse(&yaml).validate().is_ok());
}

#[test]
fn skills_are_validated_without_any_plugin() {
    let yaml = format!("skills:\n  skills:{}", skill_yaml("bad-skill"));
    assert!(parse(&yaml).validate().is_err());
}

#[test]
fn marketplace_skills_block_is_rejected_at_parse() {
    let yaml = format!(
        "marketplaces:{}",
        marketplace_yaml("market", "    skills:\n      include: [anything]\n")
    );
    let err = serde_yaml::from_str::<ServicesConfig>(&yaml).unwrap_err();
    assert!(err.to_string().contains("skills"), "{err}");
}

#[test]
fn default_marketplace_selector_must_be_enabled() {
    let yaml = format!(
        "settings:\n  default_marketplace_id: market-a\nmarketplaces:{}",
        marketplace_yaml("market-a", "    enabled: false\n")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("disabled marketplace"), "{err}");
}

#[test]
fn single_enabled_marketplace_needs_no_selector() {
    let yaml = format!(
        "marketplaces:{}{}",
        marketplace_yaml("market-a", ""),
        marketplace_yaml("market-b", "    enabled: false\n")
    );
    assert!(parse(&yaml).validate().is_ok());
}

#[test]
fn enabled_plugin_depending_on_disabled_mcp_server_is_rejected() {
    let yaml = format!(
        "mcp_servers:{}\nplugins:{}",
        mcp_yaml("dark", 5010, "internal").replace("enabled: true", "enabled: false"),
        plugin_yaml("plug", false, "{}", "{ include: [dark] }")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(
        err.to_string().contains("disabled mcp_server 'dark'"),
        "{err}"
    );
}

#[test]
fn disabled_plugin_may_depend_on_disabled_mcp_server() {
    let yaml = format!(
        "mcp_servers:{}\nplugins:{}",
        mcp_yaml("dark", 5010, "internal").replace("enabled: true", "enabled: false"),
        plugin_yaml("plug", false, "{}", "{ include: [dark] }")
            .replace("enabled: true", "enabled: false")
    );
    assert!(parse(&yaml).validate().is_ok());
}

#[test]
fn enabled_skill_depending_on_disabled_mcp_server_is_rejected() {
    let yaml = format!(
        "mcp_servers:{}\nskills:\n  skills:{}",
        mcp_yaml("dark", 5010, "internal").replace("enabled: true", "enabled: false"),
        skill_yaml("needs_dark").replace(
            "enabled: true",
            "enabled: true\n      mcp_servers:\n        include: [dark]"
        )
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(
        err.to_string().contains("disabled mcp_server 'dark'"),
        "{err}"
    );
}

#[test]
fn skill_referencing_unknown_mcp_server_is_rejected() {
    let yaml = format!(
        "skills:\n  skills:{}",
        skill_yaml("needs_ghost").replace(
            "enabled: true",
            "enabled: true\n      mcp_servers:\n        include: [ghost]"
        )
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(
        err.to_string().contains("unknown mcp_server 'ghost'"),
        "{err}"
    );
}

#[test]
fn enabled_evaluation_hook_requires_the_same_plugin_to_own_governance() {
    let orphan = plugin_yaml("score-only", false, "{}", "{}").replace(
        "hooks:\n      governance: false",
        "hooks:\n      governance: false\n      evaluation: true",
    );
    let err = parse(&format!("plugins:{orphan}"))
        .validate()
        .expect_err("evaluation cannot consume a track hook nobody installs");
    let diagnosis = err.to_string();
    assert!(diagnosis.contains("score-only"), "{diagnosis}");
    assert!(diagnosis.contains("judge: true"), "{diagnosis}");
    assert!(diagnosis.contains("governance: true"), "{diagnosis}");

    let disabled_orphan = orphan.replace("enabled: true", "enabled: false");
    let owner = plugin_yaml("active-owner", true, "{}", "{}").replace(
        "hooks:\n      governance: true",
        "hooks:\n      governance: true\n      evaluation: true",
    );
    parse(&format!("plugins:{owner}{disabled_orphan}"))
        .validate()
        .expect("a disabled plugin neither installs nor consumes session hooks");
}
