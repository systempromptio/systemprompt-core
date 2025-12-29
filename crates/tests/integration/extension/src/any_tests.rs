//! Tests for AnyExtension trait and type-erased wrappers.

use std::any::Any;

use axum::Router;
use systemprompt_extension::any::{ApiExtensionWrapper, ExtensionWrapper, SchemaExtensionWrapper};
use systemprompt_extension::prelude::*;
use systemprompt_extension::typed::{
    ApiExtensionTyped, ApiExtensionTypedDyn, SchemaDefinitionTyped, SchemaExtensionTyped,
};

// =============================================================================
// Test Extension Types
// =============================================================================

#[derive(Default, Debug)]
struct BasicExtension;

impl ExtensionType for BasicExtension {
    const ID: &'static str = "basic";
    const NAME: &'static str = "Basic Extension";
    const VERSION: &'static str = "1.0.0";
    const PRIORITY: u32 = 50;
}

impl NoDependencies for BasicExtension {}

#[derive(Default, Debug)]
struct SchemaTestExtension;

impl ExtensionType for SchemaTestExtension {
    const ID: &'static str = "schema-test";
    const NAME: &'static str = "Schema Test Extension";
    const VERSION: &'static str = "2.0.0";
    const PRIORITY: u32 = 25;
}

impl NoDependencies for SchemaTestExtension {}

impl SchemaExtensionTyped for SchemaTestExtension {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
        vec![SchemaDefinitionTyped::embedded(
            "test_table",
            "CREATE TABLE test_table (id INTEGER PRIMARY KEY)",
        )]
    }

    fn migration_weight(&self) -> u32 {
        10
    }
}

#[derive(Default, Debug)]
struct ApiTestExtension;

impl ExtensionType for ApiTestExtension {
    const ID: &'static str = "api-test";
    const NAME: &'static str = "API Test Extension";
    const VERSION: &'static str = "3.0.0";
    const PRIORITY: u32 = 75;
}

impl NoDependencies for ApiTestExtension {}

impl ApiExtensionTyped for ApiTestExtension {
    fn base_path(&self) -> &'static str {
        "/api/v1/test-ext"
    }

    fn requires_auth(&self) -> bool {
        false
    }
}

impl ApiExtensionTypedDyn for ApiTestExtension {
    fn build_router(&self) -> Router {
        Router::new()
    }
}

// =============================================================================
// ExtensionWrapper Tests
// =============================================================================

#[test]
fn test_extension_wrapper_new() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    let _ = wrapper; // Just verify it compiles and can be created
}

#[test]
fn test_extension_wrapper_id() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    assert_eq!(wrapper.id(), "basic");
}

#[test]
fn test_extension_wrapper_name() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    assert_eq!(wrapper.name(), "Basic Extension");
}

#[test]
fn test_extension_wrapper_version() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    assert_eq!(wrapper.version(), "1.0.0");
}

#[test]
fn test_extension_wrapper_priority() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    assert_eq!(wrapper.priority(), 50);
}

#[test]
fn test_extension_wrapper_as_any() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    let any_ref: &dyn Any = wrapper.as_any();

    // Should be able to downcast to the inner type
    assert!(any_ref.downcast_ref::<BasicExtension>().is_some());
}

#[test]
fn test_extension_wrapper_type_name() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    let type_name = wrapper.type_name();
    assert!(type_name.contains("BasicExtension"));
}

#[test]
fn test_extension_wrapper_as_schema_returns_none() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    assert!(wrapper.as_schema().is_none());
}

#[test]
fn test_extension_wrapper_as_api_returns_none() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    assert!(wrapper.as_api().is_none());
}

#[test]
fn test_extension_wrapper_debug() {
    let wrapper = ExtensionWrapper::new(BasicExtension);
    let debug_str = format!("{:?}", wrapper);
    assert!(debug_str.contains("ExtensionWrapper"));
    assert!(debug_str.contains("BasicExtension"));
}

// =============================================================================
// SchemaExtensionWrapper Tests
// =============================================================================

#[test]
fn test_schema_extension_wrapper_new() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    let _ = wrapper;
}

#[test]
fn test_schema_extension_wrapper_id() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    assert_eq!(wrapper.id(), "schema-test");
}

#[test]
fn test_schema_extension_wrapper_name() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    assert_eq!(wrapper.name(), "Schema Test Extension");
}

#[test]
fn test_schema_extension_wrapper_version() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    assert_eq!(wrapper.version(), "2.0.0");
}

#[test]
fn test_schema_extension_wrapper_priority() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    assert_eq!(wrapper.priority(), 25);
}

#[test]
fn test_schema_extension_wrapper_as_schema_returns_some() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    let schema = wrapper.as_schema();
    assert!(schema.is_some());

    let schema_ext = schema.expect("should have schema");
    assert_eq!(schema_ext.migration_weight(), 10);

    let schemas = schema_ext.schemas();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].table, "test_table");
}

#[test]
fn test_schema_extension_wrapper_as_api_returns_none() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    assert!(wrapper.as_api().is_none());
}

#[test]
fn test_schema_extension_wrapper_as_any() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    let any_ref: &dyn Any = wrapper.as_any();
    assert!(any_ref.downcast_ref::<SchemaTestExtension>().is_some());
}

#[test]
fn test_schema_extension_wrapper_type_name() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    let type_name = wrapper.type_name();
    assert!(type_name.contains("SchemaTestExtension"));
}

#[test]
fn test_schema_extension_wrapper_debug() {
    let wrapper = SchemaExtensionWrapper::new(SchemaTestExtension);
    let debug_str = format!("{:?}", wrapper);
    assert!(debug_str.contains("SchemaExtensionWrapper"));
    assert!(debug_str.contains("SchemaTestExtension"));
}

// =============================================================================
// ApiExtensionWrapper Tests
// =============================================================================

#[test]
fn test_api_extension_wrapper_new() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    let _ = wrapper;
}

#[test]
fn test_api_extension_wrapper_id() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    assert_eq!(wrapper.id(), "api-test");
}

#[test]
fn test_api_extension_wrapper_name() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    assert_eq!(wrapper.name(), "API Test Extension");
}

#[test]
fn test_api_extension_wrapper_version() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    assert_eq!(wrapper.version(), "3.0.0");
}

#[test]
fn test_api_extension_wrapper_priority() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    assert_eq!(wrapper.priority(), 75);
}

#[test]
fn test_api_extension_wrapper_as_api_returns_some() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    let api = wrapper.as_api();
    assert!(api.is_some());

    let api_ext = api.expect("should have api");
    assert_eq!(api_ext.base_path(), "/api/v1/test-ext");
    assert!(!api_ext.requires_auth());
}

#[test]
fn test_api_extension_wrapper_as_schema_returns_none() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    assert!(wrapper.as_schema().is_none());
}

#[test]
fn test_api_extension_wrapper_as_any() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    let any_ref: &dyn Any = wrapper.as_any();
    assert!(any_ref.downcast_ref::<ApiTestExtension>().is_some());
}

#[test]
fn test_api_extension_wrapper_type_name() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    let type_name = wrapper.type_name();
    assert!(type_name.contains("ApiTestExtension"));
}

#[test]
fn test_api_extension_wrapper_debug() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    let debug_str = format!("{:?}", wrapper);
    assert!(debug_str.contains("ApiExtensionWrapper"));
    assert!(debug_str.contains("ApiTestExtension"));
}

#[test]
fn test_api_extension_wrapper_build_router() {
    let wrapper = ApiExtensionWrapper::new(ApiTestExtension);
    let api = wrapper.as_api().expect("should have api");
    let router = api.build_router();
    // Just verify it returns a router without panicking
    let _ = router;
}

// =============================================================================
// AnyExtension Trait Object Tests
// =============================================================================

#[test]
fn test_any_extension_as_trait_object() {
    use systemprompt_extension::any::AnyExtension;

    let wrapper: Box<dyn AnyExtension> = Box::new(ExtensionWrapper::new(BasicExtension));
    assert_eq!(wrapper.id(), "basic");
    assert_eq!(wrapper.name(), "Basic Extension");
    assert_eq!(wrapper.version(), "1.0.0");
    assert_eq!(wrapper.priority(), 50);
}

#[test]
fn test_schema_any_extension_as_trait_object() {
    use systemprompt_extension::any::AnyExtension;

    let wrapper: Box<dyn AnyExtension> = Box::new(SchemaExtensionWrapper::new(SchemaTestExtension));
    assert_eq!(wrapper.id(), "schema-test");
    assert!(wrapper.as_schema().is_some());
}

#[test]
fn test_api_any_extension_as_trait_object() {
    use systemprompt_extension::any::AnyExtension;

    let wrapper: Box<dyn AnyExtension> = Box::new(ApiExtensionWrapper::new(ApiTestExtension));
    assert_eq!(wrapper.id(), "api-test");
    assert!(wrapper.as_api().is_some());
}

// =============================================================================
// Default Trait Method Tests
// =============================================================================

#[test]
fn test_any_extension_default_as_config_returns_none() {
    use systemprompt_extension::any::AnyExtension;

    let wrapper: Box<dyn AnyExtension> = Box::new(ExtensionWrapper::new(BasicExtension));
    assert!(wrapper.as_config().is_none());
}

#[test]
fn test_any_extension_default_as_job_returns_none() {
    use systemprompt_extension::any::AnyExtension;

    let wrapper: Box<dyn AnyExtension> = Box::new(ExtensionWrapper::new(BasicExtension));
    assert!(wrapper.as_job().is_none());
}

#[test]
fn test_any_extension_default_as_provider_returns_none() {
    use systemprompt_extension::any::AnyExtension;

    let wrapper: Box<dyn AnyExtension> = Box::new(ExtensionWrapper::new(BasicExtension));
    assert!(wrapper.as_provider().is_none());
}
