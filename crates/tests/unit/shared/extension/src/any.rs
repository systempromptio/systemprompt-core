use systemprompt_extension::any::{AnyExtension, ExtensionWrapper, SchemaExtensionWrapper};
use systemprompt_extension::typed::{SchemaDefinitionTyped, SchemaExtensionTyped};
use systemprompt_extension::types::{ExtensionType, NoDependencies};

#[derive(Debug, Default)]
struct SimpleExt;

impl ExtensionType for SimpleExt {
    const ID: &'static str = "simple";
    const NAME: &'static str = "Simple Extension";
    const VERSION: &'static str = "0.1.0";
    const PRIORITY: u32 = 75;
}

impl NoDependencies for SimpleExt {}

#[derive(Debug, Default)]
struct SchemaExt;

impl ExtensionType for SchemaExt {
    const ID: &'static str = "schema-ext";
    const NAME: &'static str = "Schema Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for SchemaExt {}

impl SchemaExtensionTyped for SchemaExt {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
        vec![SchemaDefinitionTyped::embedded(
            "test_table",
            "CREATE TABLE test_table (id TEXT PRIMARY KEY)",
        )]
    }

    fn migration_weight(&self) -> u32 {
        50
    }
}

#[test]
fn extension_wrapper_id() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    assert_eq!(wrapper.id(), "simple");
}

#[test]
fn extension_wrapper_name() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    assert_eq!(wrapper.name(), "Simple Extension");
}

#[test]
fn extension_wrapper_version() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    assert_eq!(wrapper.version(), "0.1.0");
}

#[test]
fn extension_wrapper_priority() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    assert_eq!(wrapper.priority(), 75);
}

#[test]
fn extension_wrapper_as_schema_returns_none() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    assert!(wrapper.as_schema().is_none());
}

#[test]
fn extension_wrapper_as_config_returns_none() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    assert!(wrapper.as_config().is_none());
}

#[test]
fn extension_wrapper_as_job_returns_none() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    assert!(wrapper.as_job().is_none());
}

#[test]
fn extension_wrapper_as_provider_returns_none() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    assert!(wrapper.as_provider().is_none());
}

#[test]
fn extension_wrapper_as_any_downcast() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    let any = wrapper.as_any();
    assert!(any.downcast_ref::<SimpleExt>().is_some());
}

#[test]
fn extension_wrapper_type_name_contains_struct_name() {
    let wrapper = ExtensionWrapper::new(SimpleExt);
    let name = wrapper.type_name();
    assert!(name.contains("SimpleExt"));
}

#[test]
fn schema_extension_wrapper_id() {
    let wrapper = SchemaExtensionWrapper::new(SchemaExt);
    assert_eq!(wrapper.id(), "schema-ext");
}

#[test]
fn schema_extension_wrapper_as_schema_returns_some() {
    let wrapper = SchemaExtensionWrapper::new(SchemaExt);
    let schema = wrapper.as_schema();
    assert!(schema.is_some());
}

#[test]
fn schema_extension_wrapper_schemas_content() {
    let wrapper = SchemaExtensionWrapper::new(SchemaExt);
    let schema_trait = wrapper.as_schema().expect("should have schema");
    let schemas = schema_trait.schemas();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0].table, "test_table");
}

#[test]
fn schema_extension_wrapper_migration_weight() {
    let wrapper = SchemaExtensionWrapper::new(SchemaExt);
    let schema_trait = wrapper.as_schema().expect("should have schema");
    assert_eq!(schema_trait.migration_weight(), 50);
}

#[test]
fn schema_extension_wrapper_as_any_downcast() {
    let wrapper = SchemaExtensionWrapper::new(SchemaExt);
    let any = wrapper.as_any();
    assert!(any.downcast_ref::<SchemaExt>().is_some());
}
