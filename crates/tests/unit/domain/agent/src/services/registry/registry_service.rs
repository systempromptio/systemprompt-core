use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_identifiers::AgentName;
use systemprompt_test_fixtures::ensure_test_bootstrap;


#[tokio::test]
async fn agent_registry_list_agents_empty_default() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new(&systemprompt_test_fixtures::fixture_services_state());
    let list = registry.list_agents();
    assert!(list.is_empty());
}

#[tokio::test]
async fn agent_registry_list_enabled_empty_default() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new(&systemprompt_test_fixtures::fixture_services_state());
    let list = registry.list_enabled_agents().expect("list");
    assert!(list.is_empty());
}

#[tokio::test]
async fn agent_registry_get_agent_unknown_errors() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new(&systemprompt_test_fixtures::fixture_services_state());
    let err = registry
        .get_agent("__no_such_agent_aaa")
        .expect_err("missing agent");
    assert!(format!("{err}").contains("__no_such_agent_aaa"));
}

#[tokio::test]
async fn agent_registry_get_default_errors_when_unset() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new(&systemprompt_test_fixtures::fixture_services_state());
    let err = registry.get_default_agent().expect_err("no default");
    assert!(format!("{err}").contains("default"));
}

#[tokio::test]
async fn agent_registry_get_mcp_servers_unknown_errors() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new(&systemprompt_test_fixtures::fixture_services_state());
    let result = registry.get_mcp_servers(&AgentName::new("__no_agent_for_mcp"));
    assert!(result.is_err());
}

#[tokio::test]
async fn agent_registry_find_next_available_port_returns_base() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new(&systemprompt_test_fixtures::fixture_services_state());
    let port = registry.find_next_available_port().expect("next port");
    assert!(port >= 9000);
    assert!(port <= 9999);
}

#[tokio::test]
async fn agent_registry_clone_independent() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new(&systemprompt_test_fixtures::fixture_services_state());
    let cloned = registry.clone();
    let original_list = registry.list_agents();
    let cloned_list = cloned.list_agents();
    assert_eq!(original_list.len(), cloned_list.len());
}

#[tokio::test]
async fn agent_registry_to_agent_card_unknown_errors() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new(&systemprompt_test_fixtures::fixture_services_state());
    let result = registry.to_agent_card("does_not_exist_xx", "http://test", vec![], None);
    assert!(result.is_err());
}
