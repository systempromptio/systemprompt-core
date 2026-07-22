// Drives AgentRegistryProviderService over an injected registry snapshot:
// get_agent maps AgentConfig into the trait-level AgentInfo (including the
// OAuth projection), list_enabled_agents filters disabled entries, and the
// default-agent lookup distinguishes flagged and unflagged snapshots.

use std::collections::HashMap;

use systemprompt_agent::services::AgentRegistryProviderService;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::{AgentConfig, ServicesConfig};
use systemprompt_traits::{AgentRegistryProvider, RegistryError};

use super::a2a_server::a2a_helpers::agent_config;

fn provider(agents: Vec<AgentConfig>) -> AgentRegistryProviderService {
    let config = ServicesConfig {
        agents: agents
            .into_iter()
            .map(|a| (a.name.clone(), a))
            .collect::<HashMap<_, _>>(),
        ..Default::default()
    };
    AgentRegistryProviderService::from_registry(AgentRegistry::from_config(config))
}

#[tokio::test]
async fn get_agent_projects_agent_info_with_oauth() {
    let provider = provider(vec![agent_config("rp_alpha")]);

    let info = provider.get_agent("rp_alpha").await.expect("agent found");
    assert_eq!(info.name, "rp_alpha");
    assert_eq!(info.port, 9100);
    assert!(info.enabled);
    assert_eq!(
        info.oauth.required,
        systemprompt_models::AgentOAuthConfig::default().required
    );
}

#[tokio::test]
async fn get_agent_unknown_maps_to_not_found() {
    let provider = provider(vec![]);
    let err = provider.get_agent("rp_ghost").await.expect_err("missing");
    assert!(matches!(err, RegistryError::NotFound(_)));
}

#[tokio::test]
async fn list_enabled_agents_excludes_disabled() {
    let mut disabled = agent_config("rp_disabled");
    disabled.enabled = false;
    let provider = provider(vec![agent_config("rp_on"), disabled]);

    let listed = provider.list_enabled_agents().await.expect("list");
    let names: Vec<_> = listed.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, vec!["rp_on"]);
}

#[tokio::test]
async fn default_agent_resolution_follows_default_flag() {
    let provider_without = provider(vec![agent_config("rp_plain")]);
    let err = provider_without
        .get_default_agent()
        .await
        .expect_err("no default flagged");
    assert!(matches!(err, RegistryError::NotFound(_)));

    let mut flagged = agent_config("rp_default");
    flagged.default = true;
    let provider_with = provider(vec![agent_config("rp_plain"), flagged]);
    let info = provider_with.get_default_agent().await.expect("default");
    assert_eq!(info.name, "rp_default");
}
