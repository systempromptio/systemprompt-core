//! Unit tests for network routing helpers.
//!
//! `apply_cors_layer` is excluded because it requires a globally-initialised
//! `Config`, which the unit-test harness does not provide.

use systemprompt_mcp::services::network::routing::{
    add_middleware, create_base_router, create_mcp_router,
};

#[test]
fn test_create_base_router_returns_router() {
    let _router = create_base_router();
}

#[test]
fn test_create_mcp_router_nests() {
    let base = create_base_router();
    let mcp = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
    let _composed = create_mcp_router(base, mcp);
}

#[test]
fn test_add_middleware_passthrough() {
    let router = axum::Router::<()>::new();
    let _passed = add_middleware(router);
}
