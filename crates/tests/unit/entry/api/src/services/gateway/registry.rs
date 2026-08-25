use systemprompt_ai::SafetyConfig;
use systemprompt_api::services::gateway::registry::{
    GatewayUpstreamRegistry, SafetyScannerRegistry,
};

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

#[test]
fn the_registry_enumerates_the_tags_it_can_dispatch() {
    let tags = GatewayUpstreamRegistry::global().tags();

    for expected in ["anthropic", "openai-chat", "openai-responses", "gemini"] {
        assert!(
            tags.contains(&expected),
            "the enumerated tags must match what `get` resolves; {expected} is missing: {tags:?}"
        );
    }
}

#[test]
fn every_enumerated_tag_resolves_to_an_adapter() {
    let registry = GatewayUpstreamRegistry::global();

    for tag in registry.tags() {
        assert!(
            registry.get(tag).is_some(),
            "a tag the registry advertises must be dispatchable: {tag}"
        );
    }
}

#[test]
fn the_safety_registry_carries_both_builtins() {
    let registry = SafetyScannerRegistry::global();
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

#[test]
fn an_unknown_scanner_name_resolves_to_nothing() {
    // A policy naming a scanner that was never registered must fail to resolve
    // rather than silently falling back to a permissive one.
    assert!(
        SafetyScannerRegistry::global()
            .create("no-such-scanner", &SafetyConfig::default())
            .is_none()
    );
}

#[test]
fn every_named_safety_scanner_resolves() {
    let registry = SafetyScannerRegistry::global();

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
