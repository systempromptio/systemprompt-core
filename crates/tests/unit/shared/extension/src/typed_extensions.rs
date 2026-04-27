use std::path::PathBuf;

use systemprompt_extension::error::ConfigError;
use systemprompt_extension::typed::{
    ApiExtensionTyped, ConfigExtensionTyped, SchemaDefinitionTyped, SchemaExtensionTyped,
    SchemaSourceTyped,
};
use systemprompt_extension::types::{ExtensionType, NoDependencies};

#[derive(Debug, Default)]
struct TestSchemaExt;

impl ExtensionType for TestSchemaExt {
    const ID: &'static str = "typed-schema";
    const NAME: &'static str = "Typed Schema";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for TestSchemaExt {}

impl SchemaExtensionTyped for TestSchemaExt {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
        vec![
            SchemaDefinitionTyped::embedded("users", "CREATE TABLE users (id TEXT)"),
            SchemaDefinitionTyped::file("posts", PathBuf::from("posts.sql")),
        ]
    }
}

#[derive(Debug, Default)]
struct TestConfigExt;

impl ExtensionType for TestConfigExt {
    const ID: &'static str = "typed-config";
    const NAME: &'static str = "Typed Config";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for TestConfigExt {}

impl ConfigExtensionTyped for TestConfigExt {
    fn config_prefix(&self) -> &'static str {
        "myconfig"
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<(), ConfigError> {
        if config.get("port").is_none() {
            return Err(ConfigError::InvalidValue {
                key: "port".to_string(),
                message: "port is required".to_string(),
            });
        }
        Ok(())
    }

    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"type": "object"}))
    }
}

#[derive(Debug, Default)]
struct TestApiExt;

impl ExtensionType for TestApiExt {
    const ID: &'static str = "typed-api";
    const NAME: &'static str = "Typed API";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for TestApiExt {}

impl ApiExtensionTyped for TestApiExt {
    fn base_path(&self) -> &'static str {
        "/api/v2/widgets"
    }
}

#[test]
fn schema_definition_typed_embedded() {
    let schema = SchemaDefinitionTyped::embedded("test", "CREATE TABLE test (id INT)");
    assert_eq!(schema.table, "test");
    assert!(matches!(schema.sql, SchemaSourceTyped::Embedded(ref s) if s.contains("CREATE")));
    assert!(schema.required_columns.is_empty());
}

#[test]
fn schema_definition_typed_file() {
    let schema = SchemaDefinitionTyped::file("items", PathBuf::from("items.sql"));
    assert_eq!(schema.table, "items");
    assert!(matches!(schema.sql, SchemaSourceTyped::File(_)));
}

#[test]
fn schema_definition_typed_with_required_columns() {
    let schema = SchemaDefinitionTyped::embedded("data", "CREATE TABLE data (id TEXT, name TEXT)")
        .with_required_columns(vec!["id".to_string(), "name".to_string()]);
    assert_eq!(schema.required_columns.len(), 2);
}

#[test]
fn schema_definition_typed_serde_roundtrip() {
    let schema = SchemaDefinitionTyped::embedded("events", "CREATE TABLE events (id TEXT)");
    let json = serde_json::to_string(&schema).expect("serialize");
    let deserialized: SchemaDefinitionTyped = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.table, "events");
}

#[test]
fn schema_source_typed_embedded_variant() {
    let source = SchemaSourceTyped::Embedded("SELECT 1".to_string());
    assert!(matches!(source, SchemaSourceTyped::Embedded(ref s) if s == "SELECT 1"));
}

#[test]
fn schema_source_typed_file_variant() {
    let source = SchemaSourceTyped::File(PathBuf::from("test.sql"));
    assert!(matches!(source, SchemaSourceTyped::File(ref p) if p.to_str().unwrap() == "test.sql"));
}

#[test]
fn schema_extension_typed_schemas_returns_definitions() {
    let ext = TestSchemaExt;
    let schemas = ext.schemas();
    assert_eq!(schemas.len(), 2);
    assert_eq!(schemas[0].table, "users");
    assert_eq!(schemas[1].table, "posts");
}

#[test]
fn schema_extension_typed_default_migration_weight() {
    #[derive(Debug, Default)]
    struct DefaultWeightExt;

    impl ExtensionType for DefaultWeightExt {
        const ID: &'static str = "default-weight";
        const NAME: &'static str = "Default Weight";
        const VERSION: &'static str = "1.0.0";
    }

    impl NoDependencies for DefaultWeightExt {}

    impl SchemaExtensionTyped for DefaultWeightExt {
        fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
            vec![]
        }
    }

    assert_eq!(DefaultWeightExt.migration_weight(), 100);
}

#[test]
fn config_extension_typed_prefix() {
    let ext = TestConfigExt;
    assert_eq!(ext.config_prefix(), "myconfig");
}

#[test]
fn config_extension_typed_validate_success() {
    let ext = TestConfigExt;
    let config = serde_json::json!({"port": 8080});
    assert!(ext.validate_config(&config).is_ok());
}

#[test]
fn config_extension_typed_validate_failure() {
    let ext = TestConfigExt;
    let config = serde_json::json!({"host": "localhost"});
    assert!(ext.validate_config(&config).is_err());
}

#[test]
fn config_extension_typed_schema() {
    let ext = TestConfigExt;
    let schema = ext.config_schema();
    assert!(schema.is_some());
    assert_eq!(schema.unwrap()["type"], "object");
}

#[test]
fn api_extension_typed_base_path() {
    let ext = TestApiExt;
    assert_eq!(ext.base_path(), "/api/v2/widgets");
}

#[test]
fn api_extension_typed_default_requires_auth() {
    let ext = TestApiExt;
    assert!(ext.requires_auth());
}
