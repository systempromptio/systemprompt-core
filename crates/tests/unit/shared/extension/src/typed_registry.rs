use systemprompt_extension::any::{AnyExtension, ExtensionWrapper, SchemaExtensionWrapper};
use systemprompt_extension::typed::{SchemaDefinitionTyped, SchemaExtensionTyped};
use systemprompt_extension::typed_registry::{RESERVED_PATHS, TypedExtensionRegistry};
use systemprompt_extension::types::{ExtensionMeta, ExtensionType, NoDependencies};

#[derive(Debug, Default)]
struct RegExt;

impl ExtensionType for RegExt {
    const ID: &'static str = "reg-ext";
    const NAME: &'static str = "Registry Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for RegExt {}

#[derive(Debug, Default)]
struct RegSchemaExt;

impl ExtensionType for RegSchemaExt {
    const ID: &'static str = "reg-schema";
    const NAME: &'static str = "Registry Schema Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for RegSchemaExt {}

impl SchemaExtensionTyped for RegSchemaExt {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
        vec![SchemaDefinitionTyped::embedded("reg_table", "CREATE TABLE reg_table (id TEXT)")]
    }
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
    let result = registry.validate_api_path("test-ext", "/api/v2/custom");
    assert!(result.is_ok());
}

#[test]
fn typed_registry_validate_api_path_accepts_dot_prefix() {
    let registry = TypedExtensionRegistry::new();
    let result = registry.validate_api_path("test-ext", "/.custom/path");
    assert!(result.is_ok());
}

#[test]
fn typed_registry_validate_api_path_rejects_reserved() {
    let registry = TypedExtensionRegistry::new();
    let result = registry.validate_api_path("test-ext", "/api/v1/oauth");
    assert!(result.is_err());
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
