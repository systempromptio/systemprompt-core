//! Tests for core extension types: SchemaDefinition and ExtensionMetadata.

use systemprompt_extension::{ExtensionMetadata, SchemaDefinition};

#[test]
fn test_extension_metadata_creation() {
    let metadata = ExtensionMetadata {
        id: "test-ext",
        name: "Test Extension",
        version: "1.0.0",
    };

    assert_eq!(metadata.id, "test-ext");
    assert_eq!(metadata.name, "Test Extension");
    assert_eq!(metadata.version, "1.0.0");
}

#[test]
fn test_extension_metadata_debug() {
    let metadata = ExtensionMetadata {
        id: "my-ext",
        name: "My Extension",
        version: "2.0.0",
    };

    let debug_str = format!("{:?}", metadata);
    assert!(debug_str.contains("my-ext"));
    assert!(debug_str.contains("My Extension"));
    assert!(debug_str.contains("2.0.0"));
}

#[test]
fn test_extension_metadata_serialize() {
    let metadata = ExtensionMetadata {
        id: "ser-ext",
        name: "Serializable Extension",
        version: "3.0.0",
    };

    let json = serde_json::to_string(&metadata).expect("should serialize");
    assert!(json.contains("ser-ext"));
    assert!(json.contains("Serializable Extension"));
    assert!(json.contains("3.0.0"));
}

#[test]
fn test_schema_definition_new() {
    let schema = SchemaDefinition::new("users", "CREATE TABLE users (id INTEGER PRIMARY KEY)");

    assert_eq!(schema.table.as_deref(), Some("users"));
    assert!(schema.required_columns.is_empty());
    assert!(schema.sql.contains("CREATE TABLE users"));
}

#[test]
fn test_schema_definition_with_required_columns() {
    let schema = SchemaDefinition::new("products", "CREATE TABLE products (id INT, name TEXT)")
        .with_required_columns(vec!["id".to_string(), "name".to_string()]);

    assert_eq!(schema.table.as_deref(), Some("products"));
    assert_eq!(schema.required_columns.len(), 2);
    assert!(schema.required_columns.contains(&"id".to_string()));
    assert!(schema.required_columns.contains(&"name".to_string()));
}

#[test]
fn test_schema_definition_chained_required_columns() {
    let schema =
        SchemaDefinition::new("events", "CREATE TABLE events ()").with_required_columns(vec![
            "id".to_string(),
            "timestamp".to_string(),
            "type".to_string(),
        ]);

    assert_eq!(schema.required_columns.len(), 3);
}

#[test]
fn test_schema_definition_empty_required_columns() {
    let schema =
        SchemaDefinition::new("empty", "CREATE TABLE empty ()").with_required_columns(vec![]);

    assert!(schema.required_columns.is_empty());
}

#[test]
fn test_multiple_schema_definitions() {
    let schemas = [
        SchemaDefinition::new("users", "CREATE TABLE users ()"),
        SchemaDefinition::new("posts", "CREATE TABLE posts ()"),
        SchemaDefinition::new("comments", "CREATE TABLE comments ()"),
    ];

    assert_eq!(schemas.len(), 3);
    assert_eq!(schemas[0].table.as_deref(), Some("users"));
    assert_eq!(schemas[1].table.as_deref(), Some("posts"));
    assert_eq!(schemas[2].table.as_deref(), Some("comments"));
}
