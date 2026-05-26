use systemprompt_identifiers::{AgentId, SkillId};
use systemprompt_models::services::{
    AgentCardConfig, AgentSkillConfig, CapabilitiesConfig, DiskAgentConfig, OAuthConfig,
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
        skills: vec![AgentSkillConfig {
            id: SkillId::new("skill_a"),
            name: "Skill A".to_owned(),
            description: "desc".to_owned(),
            tags: vec![],
            examples: None,
            input_modes: None,
            output_modes: None,
            security: None,
        }],
        supports_authenticated_extended_card: false,
    }
}

fn valid_disk(name: &str) -> DiskAgentConfig {
    DiskAgentConfig {
        id: None,
        name: name.to_owned(),
        display_name: "Display".to_owned(),
        description: "Desc".to_owned(),
        version: "1.0.0".to_owned(),
        enabled: true,
        port: 9000,
        endpoint: None,
        dev_only: false,
        is_primary: false,
        default: false,
        system_prompt_file: None,
        tags: vec![],
        category: None,
        mcp_servers: vec!["fs".to_owned()],
        skills: vec!["skill_a".to_owned()],
        provider: Some("anthropic".to_owned()),
        model: Some("claude".to_owned()),
        card: empty_card(),
        oauth: OAuthConfig::default(),
    }
}

#[test]
fn system_prompt_file_defaults_when_unset_or_empty() {
    let cfg = valid_disk("agent_one");
    assert_eq!(cfg.system_prompt_file(), "system_prompt.md");

    let cfg = DiskAgentConfig {
        system_prompt_file: Some(String::new()),
        ..valid_disk("agent_one")
    };
    assert_eq!(cfg.system_prompt_file(), "system_prompt.md");

    let cfg = DiskAgentConfig {
        system_prompt_file: Some("custom.md".to_owned()),
        ..valid_disk("agent_one")
    };
    assert_eq!(cfg.system_prompt_file(), "custom.md");
}

#[test]
fn to_agent_config_synthesizes_endpoint_when_absent() {
    let cfg = valid_disk("agent_one");
    let runtime = cfg.to_agent_config("https://api.example.com/", None);
    assert_eq!(runtime.endpoint, "https://api.example.com/api/v1/agents/agent_one");
    assert_eq!(runtime.name, "agent_one");
    assert_eq!(runtime.port, 9000);
    assert_eq!(runtime.card.name.as_deref(), Some("Display"));
    assert_eq!(runtime.metadata.mcp_servers, vec!["fs".to_owned()]);
    assert_eq!(runtime.metadata.provider.as_deref(), Some("anthropic"));
    assert_eq!(runtime.metadata.model.as_deref(), Some("claude"));
    assert_eq!(runtime.metadata.system_prompt, None);
}

#[test]
fn to_agent_config_preserves_explicit_endpoint() {
    let cfg = DiskAgentConfig {
        endpoint: Some("/custom".to_owned()),
        ..valid_disk("agent_one")
    };
    let runtime = cfg.to_agent_config("https://api.example.com", Some("prompt".to_owned()));
    assert_eq!(runtime.endpoint, "/custom");
    assert_eq!(runtime.metadata.system_prompt.as_deref(), Some("prompt"));
}

#[test]
fn to_agent_config_uses_card_name_when_set() {
    let mut cfg = valid_disk("agent_one");
    cfg.card.name = Some("Friendly Name".to_owned());
    let runtime = cfg.to_agent_config("https://api.example.com", None);
    assert_eq!(runtime.card.name.as_deref(), Some("Friendly Name"));
}

#[test]
fn validate_accepts_well_formed_config() {
    let cfg = valid_disk("agent_one");
    assert!(cfg.validate("agent_one").is_ok());
}

#[test]
fn validate_rejects_id_dir_mismatch() {
    let cfg = DiskAgentConfig {
        id: Some(AgentId::new("other")),
        ..valid_disk("agent_one")
    };
    let err = cfg.validate("agent_one").unwrap_err();
    assert!(format!("{err}").contains("does not match"));
}

#[test]
fn validate_accepts_matching_id_and_dir() {
    let cfg = DiskAgentConfig {
        id: Some(AgentId::new("agent_one")),
        ..valid_disk("agent_one")
    };
    assert!(cfg.validate("agent_one").is_ok());
}

#[test]
fn validate_rejects_invalid_name_chars() {
    let cfg = valid_disk("My-Agent");
    let err = cfg.validate("My-Agent").unwrap_err();
    assert!(format!("{err}").contains("lowercase"));
}

#[test]
fn validate_rejects_short_name() {
    let cfg = valid_disk("a");
    let err = cfg.validate("a").unwrap_err();
    assert!(format!("{err}").contains("between 3 and 50"));
}

#[test]
fn validate_rejects_zero_port() {
    let cfg = DiskAgentConfig {
        port: 0,
        ..valid_disk("agent_one")
    };
    let err = cfg.validate("agent_one").unwrap_err();
    assert!(format!("{err}").contains("invalid port"));
}

#[test]
fn validate_rejects_empty_display_name() {
    let cfg = DiskAgentConfig {
        display_name: String::new(),
        ..valid_disk("agent_one")
    };
    let err = cfg.validate("agent_one").unwrap_err();
    assert!(format!("{err}").contains("display_name"));
}

#[test]
fn disk_agent_config_yaml_round_trip() {
    let yaml = r#"
name: my_agent
display_name: My Agent
description: An agent
port: 9001
mcp_servers: [fs]
skills: [a, b]
card:
  protocolVersion: '1.0'
  displayName: My Agent
  description: An agent
  version: '1.0.0'
  preferredTransport: JSONRPC
  defaultInputModes: ['text/plain']
  defaultOutputModes: ['text/plain']
  capabilities: {}
  skills: []
"#;
    let cfg: DiskAgentConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(cfg.name, "my_agent");
    assert_eq!(cfg.port, 9001);
    assert!(cfg.enabled);
    assert_eq!(cfg.version, "1.0.0");
    assert_eq!(cfg.mcp_servers, vec!["fs".to_owned()]);
}
