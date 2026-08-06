use systemprompt_models::services::{ServicesConfig, Settings};

fn config_yaml() -> &'static str {
    r"
settings:
  mcp_port_range: [5000, 5999]
  agent_port_range: [9000, 9999]
mcp_servers:
  systemprompt:
    type: internal
    binary: systemprompt-mcp-agent
    port: 5010
    enabled: true
    display_in_web: true
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
  upstream:
    type: external
    binary: unused
    port: 5020
    endpoint: https://example.test/mcp
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
"
}

fn parse() -> ServicesConfig {
    serde_yaml::from_str(config_yaml()).expect("services config parses")
}

#[test]
fn offset_shifts_local_mcp_ports_and_the_range_together() {
    let mut config = parse();

    config.apply_port_offset(100).expect("offset applies");

    assert_eq!(config.mcp_servers["systemprompt"].port, 5110);
    assert_eq!(config.settings.mcp_port_range, (5100, 6099));
    config
        .validate()
        .expect("shifted ports remain inside the shifted range");
}

#[test]
fn offset_leaves_external_servers_alone() {
    let mut config = parse();

    config.apply_port_offset(100).expect("offset applies");

    assert_eq!(
        config.mcp_servers["upstream"].port, 5020,
        "an external server's port names a remote listener this host never binds"
    );
}

#[test]
fn zero_offset_changes_nothing() {
    let mut config = parse();
    let before = config.mcp_servers["systemprompt"].port;

    config.apply_port_offset(0).expect("offset applies");

    assert_eq!(config.mcp_servers["systemprompt"].port, before);
    assert_eq!(config.settings.mcp_port_range, (5000, 5999));
}

#[test]
fn two_offsets_never_collide_on_a_port() {
    let (mut a, mut b) = (parse(), parse());

    a.apply_port_offset(0).expect("offset applies");
    b.apply_port_offset(100).expect("offset applies");

    assert_ne!(
        a.mcp_servers["systemprompt"].port,
        b.mcp_servers["systemprompt"].port
    );
}

#[test]
fn offset_that_overflows_a_port_is_rejected() {
    let mut config = parse();

    let err = config
        .apply_port_offset(u16::MAX)
        .expect_err("a shift past 65535 has no valid port to name");

    assert!(err.to_string().contains("65535"));
}

#[test]
fn camel_case_settings_keys_are_rejected() {
    let err = serde_yaml::from_str::<Settings>("mcpPortRange: [5000, 5999]\n")
        .expect_err("a misspelled key must not silently fall back to the default");

    assert!(err.to_string().contains("mcpPortRange"));
}
