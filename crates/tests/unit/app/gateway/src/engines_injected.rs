//! `GatewayEngines` is the one owned set of gateway engines: built at the
//! composition root, shared by handle, and distinct per instance.

use std::sync::Arc;

use systemprompt_gateway::GatewayEngines;
use systemprompt_gateway::policies::SafetyConfig;

#[test]
fn a_new_engine_set_carries_the_builtins_of_every_engine() {
    let engines = GatewayEngines::new();

    for tag in ["anthropic", "openai-chat", "openai-responses", "gemini"] {
        assert!(
            engines.upstream_registry.get(tag).is_some(),
            "missing built-in upstream tag: {tag}"
        );
    }
    let safety = SafetyConfig::default();
    assert!(engines.scanner_registry.create("null", &safety).is_some());
    assert!(
        engines
            .scanner_registry
            .create("heuristic", &safety)
            .is_some()
    );
    assert!(
        engines.override_engine.has_extensions(),
        "the inventory-registered override is collected"
    );
    assert!(
        engines.route_selector.has_selectors(),
        "the inventory-registered selector is collected"
    );
}

#[test]
fn engines_are_values_not_a_process_singleton() {
    let first = Arc::new(GatewayEngines::new());
    let second = Arc::new(GatewayEngines::new());

    assert!(
        !Arc::ptr_eq(&first.scanner_registry, &second.scanner_registry),
        "each composition root owns its own registries"
    );
    let shared = Arc::clone(&first.scanner_registry);
    assert!(Arc::ptr_eq(&shared, &first.scanner_registry));
}
