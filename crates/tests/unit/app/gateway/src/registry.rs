use systemprompt_gateway::SafetyConfig;
use systemprompt_gateway::registry::{GatewayUpstreamRegistry, SafetyScannerRegistry};

#[test]
fn built_in_protocol_tags_present() {
    let registry = GatewayUpstreamRegistry::new();
    for tag in ["anthropic", "openai-chat", "openai-responses", "gemini"] {
        assert!(registry.get(tag).is_some(), "missing built-in tag: {tag}");
    }
}

#[test]
fn provider_names_are_not_registry_keys() {
    let registry = GatewayUpstreamRegistry::new();
    assert!(registry.get("openai").is_none());
    assert!(registry.get("minimax").is_none());
}

#[test]
fn the_registry_enumerates_the_tags_it_can_dispatch() {
    let registry = GatewayUpstreamRegistry::new();
    let tags = registry.tags();

    for expected in ["anthropic", "openai-chat", "openai-responses", "gemini"] {
        assert!(
            tags.contains(&expected),
            "the enumerated tags must match what `get` resolves; {expected} is missing: {tags:?}"
        );
    }
}

#[test]
fn every_enumerated_tag_resolves_to_an_adapter() {
    let registry = GatewayUpstreamRegistry::new();

    for tag in registry.tags() {
        assert!(
            registry.get(tag).is_some(),
            "a tag the registry advertises must be dispatchable: {tag}"
        );
    }
}

#[tokio::test]
async fn the_safety_registry_carries_both_builtins() {
    let registry =
        SafetyScannerRegistry::discover(&systemprompt_test_fixtures::fixture_extension_context(
            &systemprompt_test_fixtures::test_db_pool().await,
        ));
    let safety = SafetyConfig::default();

    assert!(
        registry.create("null", &safety).is_some(),
        "the null scanner is how a deployment opts out explicitly"
    );
    assert!(
        registry.create("heuristic", &safety).is_some(),
        "heuristic is a registered builtin, constructed per policy from its SafetyConfig"
    );
}

#[tokio::test]
async fn an_unknown_scanner_name_resolves_to_nothing() {
    // A policy naming a scanner that was never registered must fail to resolve
    // rather than silently falling back to a permissive one.
    assert!(
        SafetyScannerRegistry::discover(&systemprompt_test_fixtures::fixture_extension_context(
            &systemprompt_test_fixtures::test_db_pool().await
        ))
        .create("no-such-scanner", &SafetyConfig::default())
        .is_none()
    );
}

#[tokio::test]
async fn every_named_safety_scanner_resolves() {
    let registry =
        SafetyScannerRegistry::discover(&systemprompt_test_fixtures::fixture_extension_context(
            &systemprompt_test_fixtures::test_db_pool().await,
        ));

    let safety = SafetyConfig::default();
    let names = registry.names();
    assert!(names.contains(&"null"), "{names:?}");
    assert!(names.contains(&"heuristic"), "{names:?}");
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "names() is ordered so callers can rely on it"
    );
    for name in names {
        assert!(registry.create(name, &safety).is_some(), "{name}");
    }
}
