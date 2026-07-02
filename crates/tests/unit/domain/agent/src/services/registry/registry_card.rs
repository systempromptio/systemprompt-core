// AgentRegistry over an explicit ServicesConfig: lookups, enabled/default
// filtering, port allocation, and AgentCard assembly (interfaces, transport
// selection, runtime-status extensions, oauth-derived security config).

use std::collections::HashMap;

use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::{AgentConfig, ServicesConfig};

use super::super::a2a_server::a2a_helpers::agent_config;

fn config_with(agents: Vec<AgentConfig>) -> ServicesConfig {
    let map: HashMap<String, AgentConfig> =
        agents.into_iter().map(|a| (a.name.clone(), a)).collect();
    ServicesConfig {
        agents: map,
        ..ServicesConfig::default()
    }
}

fn registry_with(agents: Vec<AgentConfig>) -> AgentRegistry {
    AgentRegistry::from_config(config_with(agents))
}

#[tokio::test]
async fn get_and_list_reflect_injected_config() {
    let mut disabled = agent_config("beta");
    disabled.enabled = false;
    let registry = registry_with(vec![agent_config("alpha"), disabled]);

    let alpha = registry.get_agent("alpha").await.expect("alpha");
    assert_eq!(alpha.name, "alpha");

    let all = registry.list_agents().await.expect("list");
    assert_eq!(all.len(), 2);

    let enabled = registry.list_enabled_agents().await.expect("enabled");
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].name, "alpha");
}

#[tokio::test]
async fn default_agent_resolution() {
    let mut default_agent = agent_config("primary");
    default_agent.default = true;
    let registry = registry_with(vec![agent_config("other"), default_agent]);

    let resolved = registry.get_default_agent().await.expect("default");
    assert_eq!(resolved.name, "primary");

    let none = registry_with(vec![agent_config("other")]);
    assert!(none.get_default_agent().await.is_err());
}

#[tokio::test]
async fn find_next_available_port_skips_used_ports() {
    let mut a = agent_config("porter");
    a.port = 9000;
    let registry = registry_with(vec![a]);
    let port = registry.find_next_available_port().await.expect("port");
    assert_eq!(port, 9001);
}

#[tokio::test]
async fn get_mcp_servers_returns_included_list() {
    let mut a = agent_config("mcp_agent");
    a.metadata.mcp_servers.include = vec!["server_one".to_owned()];
    let registry = registry_with(vec![a]);

    let servers = registry.get_mcp_servers("mcp_agent").await.expect("mcp");
    assert_eq!(servers, vec!["server_one".to_owned()]);
}

#[tokio::test]
async fn to_agent_card_builds_interfaces_and_extensions() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _skills = crate::SKILLS_FIXTURE_LOCK.read().await;

    let mut a = agent_config("card_agent");
    a.metadata.system_prompt = Some("You are card_agent.".to_owned());
    let registry = registry_with(vec![a]);

    let card = registry
        .to_agent_card(
            "card_agent",
            "https://api.example.invalid",
            Vec::new(),
            Some(("running".to_owned(), Some(9100), Some(4321))),
        )
        .await
        .expect("card");

    assert_eq!(card.name, "card_agent");
    assert_eq!(card.supported_interfaces.len(), 1);
    assert!(
        card.supported_interfaces[0]
            .url
            .starts_with("https://api.example.invalid")
    );
    let extensions = card
        .capabilities
        .extensions
        .as_ref()
        .expect("extensions present");
    assert!(extensions.len() >= 3, "identity + prompt + status");
}

#[tokio::test]
async fn to_agent_card_transport_variants() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _skills = crate::SKILLS_FIXTURE_LOCK.read().await;

    for transport in ["GRPC", "HTTP+JSON", "JSONRPC"] {
        let mut a = agent_config("transport_agent");
        a.card.preferred_transport = transport.to_owned();
        let registry = registry_with(vec![a]);
        let card = registry
            .to_agent_card("transport_agent", "http://localhost:8080", Vec::new(), None)
            .await
            .expect("card");
        assert_eq!(card.supported_interfaces.len(), 1);
    }
}

#[tokio::test]
async fn to_agent_card_unknown_agent_errors() {
    let registry = registry_with(vec![]);
    assert!(
        registry
            .to_agent_card("ghost", "http://localhost:8080", Vec::new(), None)
            .await
            .is_err()
    );
}
