use systemprompt_models::auth::JwtAudience;
use systemprompt_models::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, AgentSummary, CapabilitiesConfig,
    OAuthConfig,
};

fn empty_card() -> AgentCardConfig {
    AgentCardConfig {
        protocol_version: "1.0".to_owned(),
        name: None,
        display_name: "Display".to_owned(),
        description: "Desc".to_owned(),
        version: "1.0.0".to_owned(),
        preferred_transport: "JSONRPC".to_owned(),
        icon_url: None,
        documentation_url: None,
        provider: None,
        capabilities: CapabilitiesConfig::default(),
        default_input_modes: vec!["text/plain".to_owned()],
        default_output_modes: vec!["text/plain".to_owned()],
        security_schemes: None,
        security: None,
        skills: vec![],
        supports_authenticated_extended_card: false,
    }
}

fn valid_agent(name: &str) -> AgentConfig {
    AgentConfig {
        name: name.to_owned(),
        port: 9001,
        endpoint: "/a2a".to_owned(),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        tags: vec!["tag1".to_owned()],
        card: empty_card(),
        metadata: AgentMetadataConfig::default(),
        oauth: OAuthConfig::default(),
    }
}

#[test]
fn agent_config_validate_accepts_well_formed() {
    let a = valid_agent("my_agent");
    assert!(a.validate("my_agent").is_ok());
}

#[test]
fn agent_config_validate_rejects_key_name_mismatch() {
    let a = valid_agent("my_agent");
    let err = a.validate("other_name").unwrap_err();
    assert!(format!("{err}").contains("does not match"));
}

#[test]
fn agent_config_validate_rejects_invalid_chars() {
    let a = valid_agent("My-Agent");
    let err = a.validate("My-Agent").unwrap_err();
    assert!(format!("{err}").contains("lowercase alphanumeric"));
}

#[test]
fn agent_config_validate_rejects_short_name() {
    let a = valid_agent("a");
    let err = a.validate("a").unwrap_err();
    assert!(format!("{err}").contains("between 3 and 50"));
}

#[test]
fn agent_config_validate_rejects_zero_port() {
    let mut a = valid_agent("my_agent");
    a.port = 0;
    let err = a.validate("my_agent").unwrap_err();
    assert!(format!("{err}").contains("invalid port"));
}

#[test]
fn agent_config_construct_url_trims_trailing_slash() {
    let a = valid_agent("my_agent");
    assert_eq!(
        a.construct_url("https://example.com/"),
        "https://example.com/api/v1/agents/my_agent"
    );
    assert_eq!(
        a.construct_url("https://example.com"),
        "https://example.com/api/v1/agents/my_agent"
    );
}

#[test]
fn agent_config_extract_oauth_scopes_populates_oauth() {
    let mut a = valid_agent("my_agent");
    a.card.security = Some(vec![serde_json::json!({
        "oauth2": ["admin", "user", "service", "a2a", "mcp", "unknown_scope"]
    })]);
    a.extract_oauth_scopes_from_card();
    assert!(a.oauth.required);
    assert_eq!(a.oauth.scopes.len(), 5);
}

#[test]
fn agent_config_extract_oauth_scopes_noop_without_security_block() {
    let mut a = valid_agent("my_agent");
    a.extract_oauth_scopes_from_card();
    assert!(!a.oauth.required);
    assert!(a.oauth.scopes.is_empty());
}

#[test]
fn agent_summary_from_config_preserves_fields() {
    let mut a = valid_agent("agent_one");
    a.is_primary = true;
    a.default = true;
    let summary = AgentSummary::from_config("agent_one", &a);
    assert_eq!(summary.agent_id.as_str(), "agent_one");
    assert_eq!(summary.name, "agent_one");
    assert_eq!(summary.display_name, "Display");
    assert_eq!(summary.port, 9001);
    assert!(summary.is_primary);
    assert!(summary.is_default);
    assert_eq!(summary.tags, vec!["tag1".to_owned()]);
}

#[test]
fn agent_summary_via_from_ref_uses_config_name() {
    let a = valid_agent("agent_two");
    let summary: AgentSummary = (&a).into();
    assert_eq!(summary.name, "agent_two");
    assert_eq!(summary.agent_id.as_str(), "agent_two");
}

#[test]
fn capabilities_config_default_enables_streaming_and_history() {
    let c = CapabilitiesConfig::default();
    assert!(c.streaming);
    assert!(c.state_transition_history);
    assert!(!c.push_notifications);
}

#[test]
fn oauth_config_default_uses_a2a_audience() {
    let o = OAuthConfig::default();
    assert!(!o.required);
    assert!(o.scopes.is_empty());
    assert_eq!(o.audience, JwtAudience::A2a);
}
