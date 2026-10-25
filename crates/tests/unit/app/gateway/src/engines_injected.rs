//! `GatewayEngines` is the one owned set of gateway engines: built at the
//! composition root, shared by handle, and distinct per instance.

use std::sync::Arc;

use systemprompt_gateway::GatewayEngines;
use systemprompt_gateway::policies::SafetyConfig;

#[test]
fn a_new_engine_set_carries_the_builtins_of_every_engine() {
    let engines = GatewayEngines::new(Default::default()).expect("gateway engines build");

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
    let first = Arc::new(GatewayEngines::new(Default::default()).expect("gateway engines build"));
    let second = Arc::new(GatewayEngines::new(Default::default()).expect("gateway engines build"));

    assert!(
        !Arc::ptr_eq(&first.scanner_registry, &second.scanner_registry),
        "each composition root owns its own registries"
    );
    let shared = Arc::clone(&first.scanner_registry);
    assert!(Arc::ptr_eq(&shared, &first.scanner_registry));
}

#[test]
fn failover_state_is_owned_per_engine_set() {
    let first = GatewayEngines::new(Default::default()).expect("gateway engines build");
    let second = GatewayEngines::new(Default::default()).expect("gateway engines build");
    let key = "p/e".to_owned();

    let _held = first.load.enter(&key);

    assert_eq!(first.load.in_flight(&key), 1);
    assert_eq!(second.load.in_flight(&key), 0);
    let settings = systemprompt_manifest::services::ResilienceSettings::default();
    assert!(!Arc::ptr_eq(
        &first.breakers.for_endpoint("p/e", &settings),
        &second.breakers.for_endpoint("p/e", &settings),
    ));
}

#[test]
fn a_breaker_opened_on_one_engine_set_leaves_the_other_closed() {
    let first = GatewayEngines::new(Default::default()).expect("gateway engines build");
    let second = GatewayEngines::new(Default::default()).expect("gateway engines build");
    let settings = systemprompt_manifest::services::ResilienceSettings::default();
    let opened = first.breakers.for_endpoint("p/e", &settings);
    let untouched = second.breakers.for_endpoint("p/e", &settings);

    for _ in 0..1_000 {
        if opened.is_open() {
            break;
        }
        opened.record_failure();
    }

    assert!(opened.is_open(), "repeated failures must open the breaker");
    assert!(
        !untouched.is_open(),
        "a breaker opened on one engine set must not trip the same endpoint on another"
    );
    assert!(
        first.breakers.for_endpoint("p/e", &settings).is_open(),
        "the opening engine set keeps its breaker open"
    );
}
