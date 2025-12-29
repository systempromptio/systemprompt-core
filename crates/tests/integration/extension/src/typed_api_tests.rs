//! Tests for typed API extension traits.

use axum::routing::get;
use axum::Router;

use systemprompt_extension::prelude::*;
use systemprompt_extension::typed::{ApiExtensionTyped, ApiExtensionTypedDyn};

// =============================================================================
// Test Extension Types
// =============================================================================

#[derive(Default, Debug)]
struct PublicApiExtension;

impl ExtensionType for PublicApiExtension {
    const ID: &'static str = "public-api";
    const NAME: &'static str = "Public API Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for PublicApiExtension {}

impl ApiExtensionTyped for PublicApiExtension {
    fn base_path(&self) -> &'static str {
        "/api/v1/public"
    }

    fn requires_auth(&self) -> bool {
        false
    }
}

impl ApiExtensionTypedDyn for PublicApiExtension {
    fn build_router(&self) -> Router {
        Router::new().route("/health", get(|| async { "ok" }))
    }
}

#[derive(Default, Debug)]
struct AuthenticatedApiExtension;

impl ExtensionType for AuthenticatedApiExtension {
    const ID: &'static str = "auth-api";
    const NAME: &'static str = "Authenticated API Extension";
    const VERSION: &'static str = "2.0.0";
}

impl NoDependencies for AuthenticatedApiExtension {}

impl ApiExtensionTyped for AuthenticatedApiExtension {
    fn base_path(&self) -> &'static str {
        "/api/v1/protected"
    }

    // Uses default requires_auth() = true
}

impl ApiExtensionTypedDyn for AuthenticatedApiExtension {
    fn build_router(&self) -> Router {
        Router::new().route("/data", get(|| async { "protected data" }))
    }
}

#[derive(Default, Debug)]
struct NestedPathApiExtension;

impl ExtensionType for NestedPathApiExtension {
    const ID: &'static str = "nested-api";
    const NAME: &'static str = "Nested Path API Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for NestedPathApiExtension {}

impl ApiExtensionTyped for NestedPathApiExtension {
    fn base_path(&self) -> &'static str {
        "/api/v1/deep/nested/path"
    }
}

impl ApiExtensionTypedDyn for NestedPathApiExtension {
    fn build_router(&self) -> Router {
        Router::new()
    }
}

// =============================================================================
// ApiExtensionTyped Trait Tests
// =============================================================================

#[test]
fn test_api_extension_typed_base_path() {
    let ext = PublicApiExtension;
    assert_eq!(ext.base_path(), "/api/v1/public");
}

#[test]
fn test_api_extension_typed_public_no_auth() {
    let ext = PublicApiExtension;
    assert!(!ext.requires_auth());
}

#[test]
fn test_api_extension_typed_default_requires_auth() {
    let ext = AuthenticatedApiExtension;
    assert!(ext.requires_auth());
}

#[test]
fn test_api_extension_typed_nested_path() {
    let ext = NestedPathApiExtension;
    assert_eq!(ext.base_path(), "/api/v1/deep/nested/path");
}

#[test]
fn test_api_extension_typed_metadata() {
    let ext = PublicApiExtension;

    assert_eq!(ext.id(), "public-api");
    assert_eq!(ext.name(), "Public API Extension");
    assert_eq!(ext.version(), "1.0.0");
}

// =============================================================================
// ApiExtensionTypedDyn Trait Tests
// =============================================================================

#[test]
fn test_api_extension_typed_dyn_build_router() {
    let ext = PublicApiExtension;
    let router = ext.build_router();
    // Just verify it returns a router without panicking
    let _ = router;
}

#[test]
fn test_api_extension_typed_dyn_authenticated_router() {
    let ext = AuthenticatedApiExtension;
    let router = ext.build_router();
    let _ = router;
}

#[test]
fn test_api_extension_typed_dyn_empty_router() {
    let ext = NestedPathApiExtension;
    let router = ext.build_router();
    let _ = router;
}

// =============================================================================
// Trait Object Tests
// =============================================================================

#[test]
fn test_api_extension_as_trait_object() {
    let ext: &dyn ApiExtensionTyped = &PublicApiExtension;
    assert_eq!(ext.base_path(), "/api/v1/public");
    assert!(!ext.requires_auth());
}

#[test]
fn test_api_extension_dyn_as_trait_object() {
    let ext: &dyn ApiExtensionTypedDyn = &AuthenticatedApiExtension;
    assert_eq!(ext.base_path(), "/api/v1/protected");
    assert!(ext.requires_auth());
    let _ = ext.build_router();
}

#[test]
fn test_api_extension_boxed_trait_object() {
    let ext: Box<dyn ApiExtensionTypedDyn> = Box::new(PublicApiExtension);
    assert_eq!(ext.base_path(), "/api/v1/public");
    let _ = ext.build_router();
}

// =============================================================================
// Multiple API Extensions Tests
// =============================================================================

#[test]
fn test_multiple_api_extensions() {
    let extensions: Vec<&dyn ApiExtensionTyped> = vec![
        &PublicApiExtension,
        &AuthenticatedApiExtension,
        &NestedPathApiExtension,
    ];

    assert_eq!(extensions.len(), 3);
    assert_eq!(extensions[0].base_path(), "/api/v1/public");
    assert_eq!(extensions[1].base_path(), "/api/v1/protected");
    assert_eq!(extensions[2].base_path(), "/api/v1/deep/nested/path");
}

#[test]
fn test_filter_public_api_extensions() {
    let extensions: Vec<&dyn ApiExtensionTyped> = vec![
        &PublicApiExtension,
        &AuthenticatedApiExtension,
        &NestedPathApiExtension,
    ];

    let public_exts: Vec<_> = extensions
        .iter()
        .filter(|e| !e.requires_auth())
        .collect();

    assert_eq!(public_exts.len(), 1);
    assert_eq!(public_exts[0].base_path(), "/api/v1/public");
}

#[test]
fn test_filter_authenticated_api_extensions() {
    let extensions: Vec<&dyn ApiExtensionTyped> = vec![
        &PublicApiExtension,
        &AuthenticatedApiExtension,
        &NestedPathApiExtension,
    ];

    let auth_exts: Vec<_> = extensions
        .iter()
        .filter(|e| e.requires_auth())
        .collect();

    assert_eq!(auth_exts.len(), 2);
}

// =============================================================================
// Path Validation Tests
// =============================================================================

#[test]
fn test_api_extension_path_starts_with_api() {
    let ext = PublicApiExtension;
    assert!(ext.base_path().starts_with("/api/"));
}

#[test]
fn test_api_extension_path_versioned() {
    let ext = AuthenticatedApiExtension;
    assert!(ext.base_path().contains("/v1/"));
}

// =============================================================================
// Custom Router Tests
// =============================================================================

#[derive(Default, Debug)]
struct MultiRouteApiExtension;

impl ExtensionType for MultiRouteApiExtension {
    const ID: &'static str = "multi-route";
    const NAME: &'static str = "Multi Route Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for MultiRouteApiExtension {}

impl ApiExtensionTyped for MultiRouteApiExtension {
    fn base_path(&self) -> &'static str {
        "/api/v1/multi"
    }
}

impl ApiExtensionTypedDyn for MultiRouteApiExtension {
    fn build_router(&self) -> Router {
        Router::new()
            .route("/list", get(|| async { "list" }))
            .route("/create", get(|| async { "create" }))
            .route("/delete", get(|| async { "delete" }))
    }
}

#[test]
fn test_api_extension_multi_route() {
    let ext = MultiRouteApiExtension;
    let router = ext.build_router();
    // Router created successfully with multiple routes
    let _ = router;
}

// =============================================================================
// Integration with Builder Tests
// =============================================================================

#[test]
fn test_api_extension_in_builder() {
    let registry = ExtensionBuilder::new()
        .api_extension(PublicApiExtension)
        .build()
        .expect("build should succeed");

    assert!(registry.has("public-api"));

    let api_extensions: Vec<_> = registry.api_extensions().collect();
    assert_eq!(api_extensions.len(), 1);
    assert_eq!(api_extensions[0].base_path(), "/api/v1/public");
}

#[test]
fn test_multiple_api_extensions_in_builder() {
    let registry = ExtensionBuilder::new()
        .api_extension(PublicApiExtension)
        .api_extension(AuthenticatedApiExtension)
        .build()
        .expect("build should succeed");

    assert!(registry.has("public-api"));
    assert!(registry.has("auth-api"));

    let api_extensions: Vec<_> = registry.api_extensions().collect();
    assert_eq!(api_extensions.len(), 2);
}

#[test]
fn test_api_extension_paths_from_registry() {
    let registry = ExtensionBuilder::new()
        .api_extension(PublicApiExtension)
        .api_extension(AuthenticatedApiExtension)
        .build()
        .expect("build should succeed");

    let paths = registry.api_paths();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"/api/v1/public".to_string()));
    assert!(paths.contains(&"/api/v1/protected".to_string()));
}
