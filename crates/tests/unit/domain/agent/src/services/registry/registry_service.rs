use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_test_fixtures::ensure_test_bootstrap;

#[tokio::test]
async fn agent_registry_new_with_empty_config_succeeds() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new().expect("registry construction");
    let dbg = format!("{:?}", registry);
    assert!(dbg.contains("AgentRegistry"));
}

#[tokio::test]
async fn agent_registry_list_agents_empty_default() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new().expect("registry");
    let list = registry.list_agents().await.expect("list");
    assert!(list.is_empty());
}

#[tokio::test]
async fn agent_registry_list_enabled_empty_default() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new().expect("registry");
    let list = registry.list_enabled_agents().await.expect("list");
    assert!(list.is_empty());
}

#[tokio::test]
async fn agent_registry_get_agent_unknown_errors() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new().expect("registry");
    let err = registry
        .get_agent("__no_such_agent_aaa")
        .await
        .expect_err("missing agent");
    assert!(format!("{err}").contains("__no_such_agent_aaa"));
}

#[tokio::test]
async fn agent_registry_get_default_errors_when_unset() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new().expect("registry");
    let err = registry.get_default_agent().await.expect_err("no default");
    assert!(format!("{err}").contains("default"));
}

#[tokio::test]
async fn agent_registry_get_mcp_servers_unknown_errors() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new().expect("registry");
    let result = registry.get_mcp_servers("__no_agent_for_mcp").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn agent_registry_find_next_available_port_returns_base() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new().expect("registry");
    let port = registry
        .find_next_available_port()
        .await
        .expect("next port");
    assert!(port >= 9000);
    assert!(port <= 9999);
}

#[tokio::test]
async fn agent_registry_clone_independent() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new().expect("registry");
    let cloned = registry.clone();
    let original_list = registry.list_agents().await.expect("list");
    let cloned_list = cloned.list_agents().await.expect("list");
    assert_eq!(original_list.len(), cloned_list.len());
}

#[tokio::test]
async fn agent_registry_to_agent_card_unknown_errors() {
    ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;
    let registry = AgentRegistry::new().expect("registry");
    let result = registry
        .to_agent_card("does_not_exist_xx", "http://test", vec![], None)
        .await;
    assert!(result.is_err());
}
