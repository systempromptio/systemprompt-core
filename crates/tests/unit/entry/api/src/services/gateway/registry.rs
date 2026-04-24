use systemprompt_api::services::gateway::registry::GatewayUpstreamRegistry;

#[test]
fn built_in_tags_present() {
    let registry = GatewayUpstreamRegistry::global();
    for tag in ["anthropic", "minimax", "openai", "moonshot", "qwen"] {
        assert!(registry.get(tag).is_some(), "missing built-in tag: {tag}");
    }
}

#[test]
fn gemini_is_not_built_in() {
    let registry = GatewayUpstreamRegistry::global();
    assert!(registry.get("gemini").is_none());
}
