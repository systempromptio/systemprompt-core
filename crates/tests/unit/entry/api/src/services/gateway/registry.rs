use systemprompt_api::services::gateway::registry::GatewayUpstreamRegistry;

#[test]
fn built_in_protocol_tags_present() {
    let registry = GatewayUpstreamRegistry::global();
    for tag in ["anthropic", "openai-chat", "openai-responses", "gemini"] {
        assert!(registry.get(tag).is_some(), "missing built-in tag: {tag}");
    }
}

#[test]
fn provider_names_are_not_registry_keys() {
    let registry = GatewayUpstreamRegistry::global();
    assert!(registry.get("openai").is_none());
    assert!(registry.get("minimax").is_none());
}
