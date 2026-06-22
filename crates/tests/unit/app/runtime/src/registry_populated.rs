//! Exercises the *populated* branches of `ModuleApiRegistry`.
//!
//! The constructor-only tests in `registry.rs` only reach the `None`
//! lookup arms because no module is registered in that test binary. Here we
//! submit a real `ModuleApiRegistration` (and a `WellKnownRoute`) via the
//! crate's inventory macros, so `ModuleApiRegistry::new()` materialises an
//! entry and the getters / category filter return `Some` / non-empty results.

use axum::Router;
use systemprompt_runtime::{
    AppContext, ModuleApiRegistry, ModuleType, ServiceCategory, get_wellknown_metadata,
    register_module_api, register_wellknown_route,
};

const TEST_MODULE: &str = "runtime-tests-fixture-module";

fn fixture_router(_ctx: &AppContext) -> Router {
    Router::new().route("/probe", axum::routing::get(|| async { "ok" }))
}

register_module_api!(
    "runtime-tests-fixture-module",
    ServiceCategory::Mcp,
    fixture_router,
    true,
    ModuleType::Proxy
);

const WELLKNOWN_PATH: &str = "/.well-known/runtime-tests-fixture";

register_wellknown_route!(
    "/.well-known/runtime-tests-fixture",
    fixture_router,
    &[axum::http::Method::GET],
    name: "runtime-tests-fixture",
    description: "A fixture well-known route registered by the runtime test suite"
);

#[test]
fn registered_module_is_materialised() {
    let registry = ModuleApiRegistry::new();

    assert_eq!(
        registry.get_category(TEST_MODULE),
        Some(ServiceCategory::Mcp),
        "category getter must return the registered category"
    );
    assert_eq!(
        registry.get_module_type(TEST_MODULE),
        Some(ModuleType::Proxy),
        "module-type getter must return the registered type"
    );
    assert_eq!(
        registry.get_auth_required(TEST_MODULE),
        Some(true),
        "auth-required getter must return the registered flag"
    );
}

#[test]
fn registered_module_appears_in_its_category() {
    let registry = ModuleApiRegistry::new();
    let mcp_modules = registry.modules_by_category(ServiceCategory::Mcp);
    assert!(
        mcp_modules.iter().any(|m| m == TEST_MODULE),
        "the registered module must show up under its own category; got {mcp_modules:?}"
    );
}

#[test]
fn registered_module_absent_from_unrelated_category() {
    let registry = ModuleApiRegistry::new();
    let agent_modules = registry.modules_by_category(ServiceCategory::Agent);
    assert!(
        !agent_modules.iter().any(|m| m == TEST_MODULE),
        "the Mcp-categorised module must not appear under Agent; got {agent_modules:?}"
    );
}

#[test]
fn registered_wellknown_metadata_is_resolvable() {
    let meta = get_wellknown_metadata(WELLKNOWN_PATH)
        .expect("registered well-known metadata must resolve");
    assert_eq!(meta.path, WELLKNOWN_PATH);
    assert_eq!(meta.name, "runtime-tests-fixture");
    assert!(
        meta.description.contains("fixture well-known route"),
        "description should round-trip through the inventory submission; got {:?}",
        meta.description
    );
}

#[test]
fn unknown_module_still_returns_none_when_registry_is_populated() {
    let registry = ModuleApiRegistry::new();
    assert!(registry.get_category("no-such-module").is_none());
    assert!(registry.get_module_type("no-such-module").is_none());
    assert!(registry.get_auth_required("no-such-module").is_none());
}
