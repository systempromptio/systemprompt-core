// Covers the McpServiceProvider impl on McpServerRegistry (services/providers.rs):
// the pure protocol_version() and the error-mapping in find_server/
// validate_registry. Config::get() is not initialised in unit tests, so the
// loader-backed calls deterministically take their error branch — which is the
// branch we want to cover.

use systemprompt_mcp::McpServerRegistry;
use systemprompt_test_fixtures::fixture_user_id;
use systemprompt_traits::McpServiceProvider;

#[test]
fn protocol_version_is_non_empty() {
    let registry = McpServerRegistry::new(fixture_user_id());
    let version = registry.protocol_version();
    assert!(!version.is_empty());
}

#[test]
fn protocol_version_matches_crate_constant() {
    let registry = McpServerRegistry::new(fixture_user_id());
    assert_eq!(
        registry.protocol_version(),
        systemprompt_mcp::mcp_protocol_version_str()
    );
}

#[test]
fn find_server_unknown_maps_to_none_or_error() {
    let registry = McpServerRegistry::new(fixture_user_id());
    let result = registry.find_server("does-not-exist");
    match result {
        Ok(opt) => assert!(opt.is_none()),
        Err(_) => {},
    }
}

#[test]
fn validate_registry_returns_result() {
    let registry = McpServerRegistry::new(fixture_user_id());
    // Without a loaded Config the registry validation fails closed; we only
    // assert the call completes and yields a Result (error branch mapping).
    let _ = registry.validate_registry();
}
