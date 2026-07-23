//! Tests for the `admin agents validate` check helpers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::agents::types::ValidationIssue;
use systemprompt_cli::admin::agents::validate::{
    ValidationSources, check_basics, check_mcp_references, check_provider,
};
use systemprompt_models::ServicesConfig;
use systemprompt_models::profile::ProviderRegistry;
use systemprompt_models::secrets::Secrets;
use systemprompt_models::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, CapabilitiesConfig, OAuthConfig,
};

fn card(display_name: &str, description: &str) -> AgentCardConfig {
    AgentCardConfig {
        protocol_version: "1.0".to_owned(),
        name: None,
        display_name: display_name.to_owned(),
        description: description.to_owned(),
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
        supports_authenticated_extended_card: false,
    }
}

fn agent(name: &str) -> AgentConfig {
    AgentConfig {
        name: name.to_owned(),
        port: 9001,
        endpoint: "/a2a".to_owned(),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        tags: vec![],
        card: card("Display", "Desc"),
        metadata: AgentMetadataConfig::default(),
        oauth: OAuthConfig::default(),
    }
}

fn services_yaml(yaml: &str) -> ServicesConfig {
    serde_yaml::from_str(yaml).unwrap()
}

fn secrets(anthropic: Option<&str>) -> Secrets {
    let mut json = serde_json::json!({
        "oauth_at_rest_pepper": "p".repeat(32),
        "database_url": "postgres://localhost/db",
    });
    if let Some(key) = anthropic {
        json["anthropic"] = serde_json::Value::String(key.to_owned());
    }
    serde_json::from_value(json).unwrap()
}

fn messages(issues: &[ValidationIssue]) -> Vec<String> {
    issues.iter().map(|i| i.message.clone()).collect()
}

#[test]
fn check_basics_flags_name_mismatch_and_empty_card_fields() {
    let mut a = agent("agent_one");
    a.card = card("", "");
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    check_basics("other_name", &a, &mut errors, &mut warnings);

    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("does not match"));
    let warn_msgs = messages(&warnings);
    assert!(warn_msgs.contains(&"Display name is empty".to_owned()));
    assert!(warn_msgs.contains(&"Description is empty".to_owned()));
    assert!(warn_msgs.contains(&"Enabled agent has no AI provider configured".to_owned()));
}

#[test]
fn check_basics_flags_zero_port_twice() {
    let mut a = agent("agent_one");
    a.port = 0;
    a.metadata.provider = Some("anthropic".to_owned());
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    check_basics("agent_one", &a, &mut errors, &mut warnings);

    let msgs = messages(&errors);
    assert!(msgs.iter().any(|m| m.contains("invalid port 0")));
    assert!(msgs.contains(&"Port cannot be 0".to_owned()));
    assert!(warnings.is_empty());
}

#[test]
fn check_basics_accepts_well_formed_agent_with_provider() {
    let mut a = agent("agent_one");
    a.metadata.provider = Some("anthropic".to_owned());
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    check_basics("agent_one", &a, &mut errors, &mut warnings);

    assert!(errors.is_empty());
    assert!(warnings.is_empty());
}

#[test]
fn check_provider_skips_disabled_and_providerless_agents() {
    let services = services_yaml("agents: {}\n");
    let registry = ProviderRegistry::default_seed().unwrap();
    let sources = ValidationSources {
        services_config: &services,
        registry: &registry,
        secrets: None,
    };
    let mut errors = Vec::new();

    let mut disabled = agent("agent_one");
    disabled.enabled = false;
    disabled.metadata.provider = Some("anthropic".to_owned());
    check_provider("agent_one", &disabled, &sources, &mut errors);

    let no_provider = agent("agent_two");
    check_provider("agent_two", &no_provider, &sources, &mut errors);

    assert!(errors.is_empty());
}

#[test]
fn check_provider_reports_unconfigured_provider() {
    let services = services_yaml("agents: {}\n");
    let registry = ProviderRegistry::default_seed().unwrap();
    let sources = ValidationSources {
        services_config: &services,
        registry: &registry,
        secrets: None,
    };
    let mut a = agent("agent_one");
    a.metadata.provider = Some("anthropic".to_owned());
    let mut errors = Vec::new();

    check_provider("agent_one", &a, &sources, &mut errors);

    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .message
            .contains("'anthropic' is not configured in ai.providers")
    );
}

#[test]
fn check_provider_reports_disabled_provider_missing_registry_entry_and_missing_key() {
    let services = services_yaml(
        "ai:\n  providers:\n    anthropic:\n      enabled: false\n    mystery:\n      enabled: true\n",
    );
    let registry = ProviderRegistry::default_seed().unwrap();
    let sources = ValidationSources {
        services_config: &services,
        registry: &registry,
        secrets: None,
    };

    let mut a = agent("agent_one");
    a.metadata.provider = Some("anthropic".to_owned());
    let mut errors = Vec::new();
    check_provider("agent_one", &a, &sources, &mut errors);
    let msgs = messages(&errors);
    assert!(msgs.iter().any(|m| m.contains("disabled in AI config")));
    assert!(
        msgs.iter()
            .any(|m| m.contains("No API key configured for provider 'anthropic'"))
    );

    let mut b = agent("agent_two");
    b.metadata.provider = Some("mystery".to_owned());
    let mut errors = Vec::new();
    check_provider("agent_two", &b, &sources, &mut errors);
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .message
            .contains("no connectivity entry in the profile registry")
    );
}

#[test]
fn check_provider_passes_when_key_present_in_secrets() {
    let services = services_yaml("ai:\n  providers:\n    anthropic:\n      enabled: true\n");
    let registry = ProviderRegistry::default_seed().unwrap();
    let secrets = secrets(Some("sk-test"));
    let sources = ValidationSources {
        services_config: &services,
        registry: &registry,
        secrets: Some(&secrets),
    };
    let mut a = agent("agent_one");
    a.metadata.provider = Some("anthropic".to_owned());
    let mut errors = Vec::new();

    check_provider("agent_one", &a, &sources, &mut errors);

    assert!(errors.is_empty());
}

#[test]
fn check_mcp_references_flags_unknown_servers_only() {
    let services = services_yaml("agents: {}\n");
    let mut a = agent("agent_one");
    a.metadata.mcp_servers.include = vec!["ghost".to_owned()];
    let mut errors = Vec::new();

    check_mcp_references("agent_one", &a, &services, &mut errors);

    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .message
            .contains("Referenced MCP server 'ghost' not found")
    );

    let clean = agent("agent_two");
    let mut errors = Vec::new();
    check_mcp_references("agent_two", &clean, &services, &mut errors);
    assert!(errors.is_empty());
}
