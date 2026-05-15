//! Tests for typed schema extension traits.

use systemprompt_extension::prelude::*;
use systemprompt_extension::typed::{SchemaDefinitionTyped, SchemaExtensionTyped};

#[test]
fn test_schema_definition_typed_new() {
    let schema = SchemaDefinitionTyped::new("users", "CREATE TABLE users (id INT PRIMARY KEY)");

    assert_eq!(schema.table, "users");
    assert!(schema.required_columns.is_empty());
    assert!(schema.sql.contains("CREATE TABLE users"));
}

#[test]
fn test_schema_definition_typed_with_required_columns() {
    let schema =
        SchemaDefinitionTyped::new("orders", "CREATE TABLE orders (id INT, customer_id INT)")
            .with_required_columns(vec!["id".to_string(), "customer_id".to_string()]);

    assert_eq!(schema.table, "orders");
    assert_eq!(schema.required_columns.len(), 2);
    assert!(schema.required_columns.contains(&"id".to_string()));
    assert!(schema.required_columns.contains(&"customer_id".to_string()));
}

#[test]
fn test_schema_definition_typed_empty_required_columns() {
    let schema =
        SchemaDefinitionTyped::new("empty", "CREATE TABLE empty ()").with_required_columns(vec![]);

    assert!(schema.required_columns.is_empty());
}

#[test]
fn test_schema_definition_typed_debug() {
    let schema = SchemaDefinitionTyped::new("debug_table", "CREATE TABLE debug_table ()");
    let debug_str = format!("{:?}", schema);

    assert!(debug_str.contains("SchemaDefinitionTyped"));
    assert!(debug_str.contains("debug_table"));
}

#[test]
fn test_schema_definition_typed_serialize() {
    let schema = SchemaDefinitionTyped::new("ser_table", "CREATE TABLE ser_table (a INT)")
        .with_required_columns(vec!["a".to_string()]);

    let json = serde_json::to_string(&schema).expect("should serialize");
    assert!(json.contains("ser_table"));
    assert!(json.contains("CREATE TABLE ser_table"));
}

#[derive(Default, Debug)]
struct TestSchemaExtension;

impl ExtensionType for TestSchemaExtension {
    const ID: &'static str = "test-schema";
    const NAME: &'static str = "Test Schema Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for TestSchemaExtension {}

impl SchemaExtensionTyped for TestSchemaExtension {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
        vec![
            SchemaDefinitionTyped::new("table_a", "CREATE TABLE table_a (id INT)"),
            SchemaDefinitionTyped::new("table_b", "CREATE TABLE table_b (id INT)"),
        ]
    }
}

#[test]
fn test_schema_extension_typed_schemas() {
    let ext = TestSchemaExtension;
    let schemas = ext.schemas();

    assert_eq!(schemas.len(), 2);
    assert_eq!(schemas[0].table, "table_a");
    assert_eq!(schemas[1].table, "table_b");
}

#[test]
fn test_schema_extension_typed_metadata() {
    let ext = TestSchemaExtension;

    assert_eq!(ext.id(), "test-schema");
    assert_eq!(ext.name(), "Test Schema Extension");
    assert_eq!(ext.version(), "1.0.0");
}

#[test]
fn test_schema_extension_with_required_columns() {
    #[derive(Default, Debug)]
    struct RequiredColumnsExtension;

    impl ExtensionType for RequiredColumnsExtension {
        const ID: &'static str = "required-cols";
        const NAME: &'static str = "Required Columns Extension";
        const VERSION: &'static str = "1.0.0";
    }

    impl NoDependencies for RequiredColumnsExtension {}

    impl SchemaExtensionTyped for RequiredColumnsExtension {
        fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
            vec![
                SchemaDefinitionTyped::new(
                    "accounts",
                    "CREATE TABLE accounts (id INT, email TEXT)",
                )
                .with_required_columns(vec!["id".to_string(), "email".to_string()]),
            ]
        }
    }

    let ext = RequiredColumnsExtension;
    let schemas = ext.schemas();

    assert_eq!(schemas[0].required_columns.len(), 2);
    assert!(schemas[0].required_columns.contains(&"id".to_string()));
    assert!(schemas[0].required_columns.contains(&"email".to_string()));
}
