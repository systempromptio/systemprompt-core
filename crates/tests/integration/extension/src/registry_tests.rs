//! Tests for the extension registry.

use systemprompt_extension::error::LoaderError;
use systemprompt_extension::prelude::*;

#[derive(Default, Debug)]
struct AuthExtension;

impl ExtensionType for AuthExtension {
    const ID: &'static str = "auth";
    const NAME: &'static str = "Authentication";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for AuthExtension {}

#[derive(Default, Debug)]
struct BlogExtension;

impl ExtensionType for BlogExtension {
    const ID: &'static str = "blog";
    const NAME: &'static str = "Blog";
    const VERSION: &'static str = "1.0.0";
}

impl Dependencies for BlogExtension {
    type Deps = (AuthExtension, ());
}

#[test]
fn test_registry_get_by_id() {
    let registry = ExtensionBuilder::new()
        .extension(AuthExtension)
        .build()
        .expect("build should succeed");

    let ext = registry.get("auth");
    assert!(ext.is_some());
    assert_eq!(ext.expect("extension exists").id(), "auth");
}

#[test]
fn test_registry_get_missing() {
    let registry = ExtensionBuilder::new()
        .extension(AuthExtension)
        .build()
        .expect("build should succeed");

    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_registry_has() {
    let registry = ExtensionBuilder::new()
        .extension(AuthExtension)
        .build()
        .expect("build should succeed");

    assert!(registry.has("auth"));
    assert!(!registry.has("blog"));
}

#[test]
fn test_registry_len() {
    let registry = ExtensionBuilder::new()
        .extension(AuthExtension)
        .extension(BlogExtension)
        .build()
        .expect("build should succeed");

    assert_eq!(registry.len(), 2);
}

#[test]
fn test_registry_is_empty() {
    let empty_registry = ExtensionBuilder::new()
        .build()
        .expect("build should succeed");
    assert!(empty_registry.is_empty());

    let non_empty = ExtensionBuilder::new()
        .extension(AuthExtension)
        .build()
        .expect("build should succeed");
    assert!(!non_empty.is_empty());
}

#[test]
fn test_registry_all_extensions() {
    let registry = ExtensionBuilder::new()
        .extension(AuthExtension)
        .extension(BlogExtension)
        .build()
        .expect("build should succeed");

    let all: Vec<_> = registry.all_extensions().collect();
    assert_eq!(all.len(), 2);
}

#[test]
fn test_registry_debug() {
    let registry = ExtensionBuilder::new()
        .extension(AuthExtension)
        .build()
        .expect("build should succeed");

    let debug_str = format!("{:?}", registry);
    assert!(debug_str.contains("TypedExtensionRegistry"));
    assert!(debug_str.contains("auth"));
}

#[test]
fn test_registry_default() {
    let registry = TypedExtensionRegistry::default();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_extension_metadata() {
    let registry = ExtensionBuilder::new()
        .extension(AuthExtension)
        .build()
        .expect("build should succeed");

    let ext = registry.get("auth").expect("auth exists");
    assert_eq!(ext.name(), "Authentication");
    assert_eq!(ext.version(), "1.0.0");
    assert_eq!(ext.priority(), 100);
}

// =============================================================================
// API Path Validation Tests
// =============================================================================

#[test]
fn test_validate_api_path_valid() {
    let registry = TypedExtensionRegistry::new();
    assert!(registry
        .validate_api_path("my-ext", "/api/v1/myext")
        .is_ok());
    assert!(registry
        .validate_api_path("my-ext", "/api/v2/something")
        .is_ok());
}

#[test]
fn test_validate_api_path_reserved_oauth() {
    let registry = TypedExtensionRegistry::new();
    let result = registry.validate_api_path("bad-ext", "/api/v1/oauth");
    assert!(matches!(
        result,
        Err(LoaderError::ReservedPathCollision { .. })
    ));
}

#[test]
fn test_validate_api_path_reserved_users() {
    let registry = TypedExtensionRegistry::new();
    let result = registry.validate_api_path("bad-ext", "/api/v1/users/profile");
    assert!(matches!(
        result,
        Err(LoaderError::ReservedPathCollision { .. })
    ));
}

#[test]
fn test_validate_api_path_reserved_admin() {
    let registry = TypedExtensionRegistry::new();
    let result = registry.validate_api_path("bad-ext", "/api/v1/admin/settings");
    assert!(matches!(
        result,
        Err(LoaderError::ReservedPathCollision { .. })
    ));
}

#[test]
fn test_validate_api_path_invalid_base() {
    let registry = TypedExtensionRegistry::new();
    let result = registry.validate_api_path("bad-ext", "/invalid/path");
    assert!(matches!(result, Err(LoaderError::InvalidBasePath { .. })));
}

#[test]
fn test_validate_api_path_well_known_reserved() {
    let registry = TypedExtensionRegistry::new();
    let result = registry.validate_api_path("bad-ext", "/.well-known/something");
    assert!(matches!(
        result,
        Err(LoaderError::ReservedPathCollision { .. })
    ));
}

#[test]
fn test_reserved_paths_constant() {
    assert!(RESERVED_PATHS.contains(&"/api/v1/oauth"));
    assert!(RESERVED_PATHS.contains(&"/api/v1/users"));
    assert!(RESERVED_PATHS.contains(&"/api/v1/admin"));
    assert!(RESERVED_PATHS.contains(&"/.well-known"));
}

#[test]
fn test_api_paths_tracking() {
    let registry = TypedExtensionRegistry::new();
    assert!(registry.api_paths().is_empty());
}
