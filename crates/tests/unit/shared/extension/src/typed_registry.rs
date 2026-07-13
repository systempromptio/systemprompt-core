use systemprompt_extension::builder::ExtensionBuilder;
use systemprompt_extension::error::LoaderError;
use systemprompt_extension::typed::{ApiExtensionTyped, ApiExtensionTypedDyn};
use systemprompt_extension::typed_registry::{RESERVED_PATHS, TypedExtensionRegistry};
use systemprompt_extension::types::{ExtensionType, NoDependencies};

#[derive(Debug, Default)]
struct WidgetsApi;

impl ExtensionType for WidgetsApi {
    const ID: &'static str = "widgets-api";
    const NAME: &'static str = "Widgets API";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for WidgetsApi {}

impl ApiExtensionTyped for WidgetsApi {
    fn base_path(&self) -> &'static str {
        "/api/v2/widgets"
    }
}

impl ApiExtensionTypedDyn for WidgetsApi {
    fn build_router(&self) -> axum::Router {
        axum::Router::new()
    }
}

fn registry_with_widgets_api() -> TypedExtensionRegistry {
    ExtensionBuilder::new()
        .api_extension(WidgetsApi)
        .build()
        .expect("build should succeed")
}

#[test]
fn typed_registry_new_is_empty() {
    let registry = TypedExtensionRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn typed_registry_default_is_empty() {
    let registry = TypedExtensionRegistry::default();
    assert!(registry.is_empty());
}

#[test]
fn typed_registry_has_returns_false_for_missing() {
    let registry = TypedExtensionRegistry::new();
    assert!(!registry.has("nonexistent"));
}

#[test]
fn typed_registry_get_returns_none_for_missing() {
    let registry = TypedExtensionRegistry::new();
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn typed_registry_api_paths_initially_empty() {
    let registry = TypedExtensionRegistry::new();
    assert!(registry.api_paths().is_empty());
}

#[test]
fn typed_registry_validate_api_path_rejects_non_api_prefix() {
    let registry = TypedExtensionRegistry::new();
    let result = registry.validate_api_path("test-ext", "/custom/path");
    assert!(result.is_err());
}

#[test]
fn typed_registry_validate_api_path_accepts_api_prefix() {
    let registry = TypedExtensionRegistry::new();
    registry
        .validate_api_path("test-ext", "/api/v2/custom")
        .expect("api prefix accepted");
}

#[test]
fn typed_registry_validate_api_path_accepts_dot_prefix() {
    let registry = TypedExtensionRegistry::new();
    registry
        .validate_api_path("test-ext", "/.custom/path")
        .expect("dot prefix accepted");
}

#[test]
fn typed_registry_validate_api_path_rejects_reserved() {
    let registry = TypedExtensionRegistry::new();
    let result = registry.validate_api_path("test-ext", "/api/v1/oauth");
    assert!(result.is_err());
}

#[test]
fn validate_api_path_rejects_subpath_of_registered_path() {
    let registry = registry_with_widgets_api();
    let err = registry
        .validate_api_path("intruder", "/api/v2/widgets/items")
        .expect_err("a sub-path of a registered base path must collide");
    match err {
        LoaderError::ReservedPathCollision { extension, path } => {
            assert_eq!(extension, "intruder");
            assert!(
                path.contains("/api/v2/widgets/items") && path.contains("conflicts with"),
                "collision must name both the offending path and the registered one: {path}"
            );
            assert!(path.contains("/api/v2/widgets"));
        },
        other => panic!("expected ReservedPathCollision, got {other:?}"),
    }
}

#[test]
fn validate_api_path_rejects_when_registered_path_is_a_prefix_of_candidate() {
    // Reversed overlap: the registered path is a prefix of the candidate's
    // stem, exercising the `existing.starts_with(path)` half of the check.
    let registry = registry_with_widgets_api();
    let err = registry
        .validate_api_path("intruder", "/api/v2/wid")
        .expect_err("an overlapping prefix must collide");
    assert!(matches!(err, LoaderError::ReservedPathCollision { .. }));
}

#[test]
fn validate_api_path_allows_disjoint_path_alongside_registered_one() {
    let registry = registry_with_widgets_api();
    registry
        .validate_api_path("neighbour", "/api/v2/gadgets")
        .expect("a disjoint path must not collide with an unrelated registered path");
}

#[test]
fn typed_registry_all_extensions_empty() {
    let registry = TypedExtensionRegistry::new();
    assert_eq!(registry.all_extensions().count(), 0);
}

#[test]
fn typed_registry_schema_extensions_empty() {
    let registry = TypedExtensionRegistry::new();
    assert_eq!(registry.schema_extensions().count(), 0);
}

#[test]
fn typed_registry_debug_format() {
    let registry = TypedExtensionRegistry::new();
    let debug = format!("{registry:?}");
    assert!(debug.contains("TypedExtensionRegistry"));
    assert!(debug.contains("count"));
}

#[test]
fn reserved_paths_contains_oauth() {
    assert!(RESERVED_PATHS.contains(&"/api/v1/oauth"));
}

#[test]
fn reserved_paths_contains_users() {
    assert!(RESERVED_PATHS.contains(&"/api/v1/users"));
}

#[test]
fn reserved_paths_contains_well_known() {
    assert!(RESERVED_PATHS.contains(&"/.well-known"));
}

#[test]
fn reserved_paths_not_empty() {
    assert!(!RESERVED_PATHS.is_empty());
}
