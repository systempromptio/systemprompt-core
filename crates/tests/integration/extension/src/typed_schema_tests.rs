//! Tests for typed schema extension traits.

use std::path::PathBuf;

use systemprompt_extension::prelude::*;
use systemprompt_extension::typed::{SchemaDefinitionTyped, SchemaExtensionTyped, SchemaSourceTyped};

// =============================================================================
// SchemaSourceTyped Tests
// =============================================================================

#[test]
fn test_schema_source_typed_embedded() {
    let source = SchemaSourceTyped::Embedded("CREATE TABLE test (id INT)".to_string());

    match source {
        SchemaSourceTyped::Embedded(sql) => {
            assert!(sql.contains("CREATE TABLE test"));
        }
        SchemaSourceTyped::File(_) => panic!("Expected Embedded variant"),
    }
}

#[test]
fn test_schema_source_typed_file() {
    let source = SchemaSourceTyped::File(PathBuf::from("/schemas/test.sql"));

    match source {
        SchemaSourceTyped::File(path) => {
            assert_eq!(path, PathBuf::from("/schemas/test.sql"));
        }
        SchemaSourceTyped::Embedded(_) => panic!("Expected File variant"),
    }
}

#[test]
fn test_schema_source_typed_debug() {
    let embedded = SchemaSourceTyped::Embedded("SELECT 1".to_string());
    let file = SchemaSourceTyped::File(PathBuf::from("test.sql"));

    let embedded_debug = format!("{:?}", embedded);
    let file_debug = format!("{:?}", file);

    assert!(embedded_debug.contains("Embedded"));
    assert!(file_debug.contains("File"));
}

#[test]
fn test_schema_source_typed_clone() {
    let source = SchemaSourceTyped::Embedded("CREATE TABLE x".to_string());
    let cloned = source.clone();

    match cloned {
        SchemaSourceTyped::Embedded(sql) => {
            assert_eq!(sql, "CREATE TABLE x");
        }
        _ => panic!("Expected Embedded variant"),
    }
}

#[test]
fn test_schema_source_typed_serialize() {
    let embedded = SchemaSourceTyped::Embedded("SELECT * FROM t".to_string());
    let json = serde_json::to_string(&embedded).expect("should serialize");
    assert!(json.contains("SELECT * FROM t"));
}

#[test]
fn test_schema_source_typed_deserialize() {
    let json = r#"{"Embedded":"CREATE TABLE y"}"#;
    let source: SchemaSourceTyped = serde_json::from_str(json).expect("should deserialize");

    match source {
        SchemaSourceTyped::Embedded(sql) => {
            assert_eq!(sql, "CREATE TABLE y");
        }
        _ => panic!("Expected Embedded variant"),
    }
}

// =============================================================================
// SchemaDefinitionTyped Tests
// =============================================================================

#[test]
fn test_schema_definition_typed_embedded() {
    let schema = SchemaDefinitionTyped::embedded("users", "CREATE TABLE users (id INT PRIMARY KEY)");

    assert_eq!(schema.table, "users");
    assert!(schema.required_columns.is_empty());

    match schema.sql {
        SchemaSourceTyped::Embedded(sql) => {
            assert!(sql.contains("CREATE TABLE users"));
        }
        _ => panic!("Expected Embedded source"),
    }
}

#[test]
fn test_schema_definition_typed_file() {
    let schema = SchemaDefinitionTyped::file("products", "/db/products.sql");

    assert_eq!(schema.table, "products");
    assert!(schema.required_columns.is_empty());

    match schema.sql {
        SchemaSourceTyped::File(path) => {
            assert_eq!(path, PathBuf::from("/db/products.sql"));
        }
        _ => panic!("Expected File source"),
    }
}

#[test]
fn test_schema_definition_typed_with_required_columns() {
    let schema = SchemaDefinitionTyped::embedded("orders", "CREATE TABLE orders (id INT, customer_id INT)")
        .with_required_columns(vec!["id".to_string(), "customer_id".to_string()]);

    assert_eq!(schema.table, "orders");
    assert_eq!(schema.required_columns.len(), 2);
    assert!(schema.required_columns.contains(&"id".to_string()));
    assert!(schema.required_columns.contains(&"customer_id".to_string()));
}

#[test]
fn test_schema_definition_typed_empty_required_columns() {
    let schema = SchemaDefinitionTyped::embedded("empty", "CREATE TABLE empty ()")
        .with_required_columns(vec![]);

    assert!(schema.required_columns.is_empty());
}

#[test]
fn test_schema_definition_typed_debug() {
    let schema = SchemaDefinitionTyped::embedded("debug_table", "CREATE TABLE debug_table ()");
    let debug_str = format!("{:?}", schema);

    assert!(debug_str.contains("SchemaDefinitionTyped"));
    assert!(debug_str.contains("debug_table"));
}

#[test]
fn test_schema_definition_typed_clone() {
    let schema = SchemaDefinitionTyped::embedded("clone_table", "CREATE TABLE clone_table ()")
        .with_required_columns(vec!["col1".to_string()]);

    let cloned = schema.clone();

    assert_eq!(cloned.table, "clone_table");
    assert_eq!(cloned.required_columns.len(), 1);
}

#[test]
fn test_schema_definition_typed_serialize() {
    let schema = SchemaDefinitionTyped::embedded("ser_table", "CREATE TABLE ser_table (a INT)")
        .with_required_columns(vec!["a".to_string()]);

    let json = serde_json::to_string(&schema).expect("should serialize");
    assert!(json.contains("ser_table"));
    assert!(json.contains("CREATE TABLE ser_table"));
}

// =============================================================================
// SchemaExtensionTyped Trait Tests
// =============================================================================

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
            SchemaDefinitionTyped::embedded("table_a", "CREATE TABLE table_a (id INT)"),
            SchemaDefinitionTyped::embedded("table_b", "CREATE TABLE table_b (id INT)"),
        ]
    }

    fn migration_weight(&self) -> u32 {
        50
    }
}

#[derive(Default, Debug)]
struct DefaultWeightExtension;

impl ExtensionType for DefaultWeightExtension {
    const ID: &'static str = "default-weight";
    const NAME: &'static str = "Default Weight Extension";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for DefaultWeightExtension {}

impl SchemaExtensionTyped for DefaultWeightExtension {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
        vec![SchemaDefinitionTyped::embedded("default_table", "CREATE TABLE default_table ()")]
    }
    // Uses default migration_weight() = 100
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
fn test_schema_extension_typed_custom_migration_weight() {
    let ext = TestSchemaExtension;
    assert_eq!(ext.migration_weight(), 50);
}

#[test]
fn test_schema_extension_typed_default_migration_weight() {
    let ext = DefaultWeightExtension;
    assert_eq!(ext.migration_weight(), 100);
}

#[test]
fn test_schema_extension_typed_metadata() {
    let ext = TestSchemaExtension;

    // ExtensionMeta is auto-implemented for ExtensionType
    assert_eq!(ext.id(), "test-schema");
    assert_eq!(ext.name(), "Test Schema Extension");
    assert_eq!(ext.version(), "1.0.0");
}

// =============================================================================
// Multiple Schema Extensions Tests
// =============================================================================

#[derive(Default, Debug)]
struct LowPrioritySchemaExt;

impl ExtensionType for LowPrioritySchemaExt {
    const ID: &'static str = "low-priority-schema";
    const NAME: &'static str = "Low Priority Schema";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for LowPrioritySchemaExt {}

impl SchemaExtensionTyped for LowPrioritySchemaExt {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
        vec![SchemaDefinitionTyped::embedded("low", "CREATE TABLE low ()")]
    }

    fn migration_weight(&self) -> u32 {
        10
    }
}

#[derive(Default, Debug)]
struct HighPrioritySchemaExt;

impl ExtensionType for HighPrioritySchemaExt {
    const ID: &'static str = "high-priority-schema";
    const NAME: &'static str = "High Priority Schema";
    const VERSION: &'static str = "1.0.0";
}

impl NoDependencies for HighPrioritySchemaExt {}

impl SchemaExtensionTyped for HighPrioritySchemaExt {
    fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
        vec![SchemaDefinitionTyped::embedded("high", "CREATE TABLE high ()")]
    }

    fn migration_weight(&self) -> u32 {
        200
    }
}

#[test]
fn test_schema_extension_ordering_by_weight() {
    let low = LowPrioritySchemaExt;
    let high = HighPrioritySchemaExt;

    let extensions: Vec<&dyn SchemaExtensionTyped> = vec![&high, &low];

    // Sort by migration weight
    let mut sorted: Vec<_> = extensions.iter().collect();
    sorted.sort_by_key(|e| e.migration_weight());

    assert_eq!(sorted[0].migration_weight(), 10);
    assert_eq!(sorted[1].migration_weight(), 200);
}

#[test]
fn test_schema_extension_with_file_source() {
    #[derive(Default, Debug)]
    struct FileSchemaExtension;

    impl ExtensionType for FileSchemaExtension {
        const ID: &'static str = "file-schema";
        const NAME: &'static str = "File Schema Extension";
        const VERSION: &'static str = "1.0.0";
    }

    impl NoDependencies for FileSchemaExtension {}

    impl SchemaExtensionTyped for FileSchemaExtension {
        fn schemas(&self) -> Vec<SchemaDefinitionTyped> {
            vec![SchemaDefinitionTyped::file("file_table", "/db/migrations/001_create_file_table.sql")]
        }
    }

    let ext = FileSchemaExtension;
    let schemas = ext.schemas();

    assert_eq!(schemas.len(), 1);
    match &schemas[0].sql {
        SchemaSourceTyped::File(path) => {
            assert!(path.to_string_lossy().contains("001_create_file_table.sql"));
        }
        _ => panic!("Expected File source"),
    }
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
                SchemaDefinitionTyped::embedded("accounts", "CREATE TABLE accounts (id INT, email TEXT)")
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
