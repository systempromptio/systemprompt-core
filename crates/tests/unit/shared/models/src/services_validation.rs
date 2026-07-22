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
    format!(
        r"
  {name}:
    server_type: {server_type}
    binary: bin
    package: null
    port: {port}
    enabled: true
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
fn external_mcp_servers_are_exempt_from_port_rules() {
    let yaml = format!(
        "agents:{}mcp_servers:{}",
        agent_yaml("agent_one", 9001, false),
        mcp_yaml("remote", 9001, "external")
    );
    assert!(parse(&yaml).validate().is_ok());
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
fn two_marketplaces_require_a_default_selector() {
    let yaml = format!(
        "marketplaces:{}{}",
        marketplace_yaml("market-a", ""),
        marketplace_yaml("market-b", "")
    );
    let err = parse(&yaml).validate().unwrap_err();
    assert!(err.to_string().contains("default_marketplace_id is unset"));
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
