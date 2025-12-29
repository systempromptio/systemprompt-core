//! Tests for core extension types: SchemaDefinition, SchemaSource, ExtensionRouter, ExtensionMetadata.

use std::path::PathBuf;

use axum::Router;
use systemprompt_extension::{ExtensionMetadata, ExtensionRouter, SchemaDefinition, SchemaSource};

// =============================================================================
// ExtensionMetadata Tests
// =============================================================================

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
fn test_extension_metadata_clone() {
    let metadata = ExtensionMetadata {
        id: "original",
        name: "Original",
        version: "1.0.0",
    };

    let cloned = metadata;
    assert_eq!(cloned.id, "original");
    assert_eq!(cloned.name, "Original");
    assert_eq!(cloned.version, "1.0.0");
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
fn test_extension_metadata_deserialize() {
    let json = r#"{"id":"deser-ext","name":"Deserializable","version":"4.0.0"}"#;
    let metadata: ExtensionMetadata = serde_json::from_str(json).expect("should deserialize");

    assert_eq!(metadata.id, "deser-ext");
    assert_eq!(metadata.name, "Deserializable");
    assert_eq!(metadata.version, "4.0.0");
}

// =============================================================================
// SchemaSource Tests
// =============================================================================

#[test]
fn test_schema_source_inline() {
    let source = SchemaSource::Inline("CREATE TABLE test (id INTEGER)".to_string());

    match source {
        SchemaSource::Inline(sql) => {
            assert!(sql.contains("CREATE TABLE test"));
        }
        SchemaSource::File(_) => panic!("Expected Inline variant"),
    }
}

#[test]
fn test_schema_source_file() {
    let source = SchemaSource::File(PathBuf::from("/path/to/schema.sql"));

    match source {
        SchemaSource::File(path) => {
            assert_eq!(path, PathBuf::from("/path/to/schema.sql"));
        }
        SchemaSource::Inline(_) => panic!("Expected File variant"),
    }
}

#[test]
fn test_schema_source_debug() {
    let inline = SchemaSource::Inline("CREATE TABLE x".to_string());
    let file = SchemaSource::File(PathBuf::from("schema.sql"));

    let inline_debug = format!("{:?}", inline);
    let file_debug = format!("{:?}", file);

    assert!(inline_debug.contains("Inline"));
    assert!(inline_debug.contains("CREATE TABLE x"));
    assert!(file_debug.contains("File"));
    assert!(file_debug.contains("schema.sql"));
}

#[test]
fn test_schema_source_clone() {
    let source = SchemaSource::Inline("CREATE TABLE y".to_string());
    let cloned = source.clone();

    match cloned {
        SchemaSource::Inline(sql) => {
            assert!(sql.contains("CREATE TABLE y"));
        }
        _ => panic!("Expected Inline variant"),
    }
}

#[test]
fn test_schema_source_serialize_inline() {
    let source = SchemaSource::Inline("SELECT 1".to_string());
    let json = serde_json::to_string(&source).expect("should serialize");
    assert!(json.contains("SELECT 1"));
}

#[test]
fn test_schema_source_serialize_file() {
    let source = SchemaSource::File(PathBuf::from("test.sql"));
    let json = serde_json::to_string(&source).expect("should serialize");
    assert!(json.contains("test.sql"));
}

// =============================================================================
// SchemaDefinition Tests
// =============================================================================

#[test]
fn test_schema_definition_inline() {
    let schema = SchemaDefinition::inline("users", "CREATE TABLE users (id INTEGER PRIMARY KEY)");

    assert_eq!(schema.table, "users");
    assert!(schema.required_columns.is_empty());

    match schema.sql {
        SchemaSource::Inline(sql) => {
            assert!(sql.contains("CREATE TABLE users"));
        }
        _ => panic!("Expected Inline source"),
    }
}

#[test]
fn test_schema_definition_file() {
    let schema = SchemaDefinition::file("orders", "/db/orders.sql");

    assert_eq!(schema.table, "orders");
    assert!(schema.required_columns.is_empty());

    match schema.sql {
        SchemaSource::File(path) => {
            assert_eq!(path, PathBuf::from("/db/orders.sql"));
        }
        _ => panic!("Expected File source"),
    }
}

#[test]
fn test_schema_definition_with_required_columns() {
    let schema = SchemaDefinition::inline("products", "CREATE TABLE products (id INT, name TEXT)")
        .with_required_columns(vec!["id".to_string(), "name".to_string()]);

    assert_eq!(schema.table, "products");
    assert_eq!(schema.required_columns.len(), 2);
    assert!(schema.required_columns.contains(&"id".to_string()));
    assert!(schema.required_columns.contains(&"name".to_string()));
}

#[test]
fn test_schema_definition_chained_required_columns() {
    let schema = SchemaDefinition::file("events", "events.sql")
        .with_required_columns(vec!["id".to_string(), "timestamp".to_string(), "type".to_string()]);

    assert_eq!(schema.required_columns.len(), 3);
}

#[test]
fn test_schema_definition_empty_required_columns() {
    let schema = SchemaDefinition::inline("empty", "CREATE TABLE empty ()")
        .with_required_columns(vec![]);

    assert!(schema.required_columns.is_empty());
}

#[test]
fn test_schema_definition_debug() {
    let schema = SchemaDefinition::inline("debug_table", "CREATE TABLE debug_table (x INT)");
    let debug_str = format!("{:?}", schema);

    assert!(debug_str.contains("SchemaDefinition"));
    assert!(debug_str.contains("debug_table"));
}

#[test]
fn test_schema_definition_clone() {
    let schema = SchemaDefinition::inline("clone_table", "CREATE TABLE clone_table ()")
        .with_required_columns(vec!["col1".to_string()]);

    let cloned = schema.clone();

    assert_eq!(cloned.table, "clone_table");
    assert_eq!(cloned.required_columns.len(), 1);
}

#[test]
fn test_schema_definition_serialize() {
    let schema = SchemaDefinition::inline("ser_table", "CREATE TABLE ser_table (a INT)")
        .with_required_columns(vec!["a".to_string()]);

    let json = serde_json::to_string(&schema).expect("should serialize");
    assert!(json.contains("ser_table"));
    assert!(json.contains("CREATE TABLE ser_table"));
}

// =============================================================================
// ExtensionRouter Tests
// =============================================================================

#[test]
fn test_extension_router_new() {
    let router = ExtensionRouter::new(Router::new(), "/api/v1/myext");

    assert_eq!(router.base_path, "/api/v1/myext");
    assert!(router.requires_auth);
}

#[test]
fn test_extension_router_public() {
    let router = ExtensionRouter::public(Router::new(), "/api/v1/public");

    assert_eq!(router.base_path, "/api/v1/public");
    assert!(!router.requires_auth);
}

#[test]
fn test_extension_router_requires_auth_default() {
    let router = ExtensionRouter::new(Router::new(), "/api/v1/auth-required");
    assert!(router.requires_auth);
}

#[test]
fn test_extension_router_public_no_auth() {
    let router = ExtensionRouter::public(Router::new(), "/api/v1/public-endpoint");
    assert!(!router.requires_auth);
}

#[test]
fn test_extension_router_debug() {
    let router = ExtensionRouter::new(Router::new(), "/api/v1/debug");
    let debug_str = format!("{:?}", router);

    assert!(debug_str.contains("ExtensionRouter"));
    assert!(debug_str.contains("/api/v1/debug"));
}

#[test]
fn test_extension_router_different_paths() {
    let router1 = ExtensionRouter::new(Router::new(), "/api/v1/ext1");
    let router2 = ExtensionRouter::new(Router::new(), "/api/v2/ext2");

    assert_eq!(router1.base_path, "/api/v1/ext1");
    assert_eq!(router2.base_path, "/api/v2/ext2");
}

#[test]
fn test_extension_router_with_nested_path() {
    let router = ExtensionRouter::new(Router::new(), "/api/v1/deep/nested/path");
    assert_eq!(router.base_path, "/api/v1/deep/nested/path");
}

// =============================================================================
// Integration Tests
// =============================================================================

#[test]
fn test_schema_definition_with_different_sources() {
    let inline_schema = SchemaDefinition::inline("t1", "CREATE TABLE t1 ()");
    let file_schema = SchemaDefinition::file("t2", "t2.sql");

    match inline_schema.sql {
        SchemaSource::Inline(_) => {}
        _ => panic!("Expected Inline"),
    }

    match file_schema.sql {
        SchemaSource::File(_) => {}
        _ => panic!("Expected File"),
    }
}

#[test]
fn test_multiple_schema_definitions() {
    let schemas = vec![
        SchemaDefinition::inline("users", "CREATE TABLE users ()"),
        SchemaDefinition::inline("posts", "CREATE TABLE posts ()"),
        SchemaDefinition::file("comments", "comments.sql"),
    ];

    assert_eq!(schemas.len(), 3);
    assert_eq!(schemas[0].table, "users");
    assert_eq!(schemas[1].table, "posts");
    assert_eq!(schemas[2].table, "comments");
}

#[test]
fn test_extension_router_collection() {
    let routers = vec![
        ExtensionRouter::new(Router::new(), "/api/v1/a"),
        ExtensionRouter::public(Router::new(), "/api/v1/b"),
        ExtensionRouter::new(Router::new(), "/api/v1/c"),
    ];

    assert!(routers[0].requires_auth);
    assert!(!routers[1].requires_auth);
    assert!(routers[2].requires_auth);
}
