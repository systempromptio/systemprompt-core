use std::path::PathBuf;

use systemprompt_extension::{
    ExtensionMetadata, ExtensionRole, SchemaDefinition, SchemaSource, SeedSource,
};

#[test]
fn extension_metadata_stores_static_fields() {
    let meta = ExtensionMetadata {
        id: "test-ext",
        name: "Test Extension",
        version: "1.0.0",
    };

    assert_eq!(meta.id, "test-ext");
    assert_eq!(meta.name, "Test Extension");
    assert_eq!(meta.version, "1.0.0");
}

#[test]
fn extension_metadata_debug_format() {
    let meta = ExtensionMetadata {
        id: "debug-ext",
        name: "Debug",
        version: "0.1.0",
    };
    let debug = format!("{meta:?}");
    assert!(debug.contains("debug-ext"));
}

#[test]
fn extension_metadata_clone() {
    let meta = ExtensionMetadata {
        id: "clone-ext",
        name: "Clone",
        version: "2.0.0",
    };
    let cloned = meta;
    assert_eq!(cloned.id, "clone-ext");
    assert_eq!(cloned.version, "2.0.0");
}

#[test]
fn extension_metadata_serde_roundtrip() {
    let meta = ExtensionMetadata {
        id: "serde-ext",
        name: "Serde Test",
        version: "3.0.0",
    };
    let json = serde_json::to_string(&meta).expect("serialize");
    assert!(json.contains("serde-ext"));
    assert!(json.contains("Serde Test"));
    assert!(json.contains("3.0.0"));
}

#[test]
fn schema_definition_inline_constructor() {
    let schema = SchemaDefinition::inline("users", "CREATE TABLE users (id TEXT)");
    assert_eq!(schema.table, "users");
    assert!(matches!(schema.sql, SchemaSource::Inline(ref s) if s.contains("CREATE TABLE")));
    assert!(schema.required_columns.is_empty());
}

#[test]
fn schema_definition_file_constructor() {
    let schema = SchemaDefinition::file("users", PathBuf::from("/path/to/schema.sql"));
    assert_eq!(schema.table, "users");
    assert!(matches!(schema.sql, SchemaSource::File(ref p) if p.to_str().unwrap().contains("schema.sql")));
}

#[test]
fn schema_definition_with_required_columns() {
    let schema = SchemaDefinition::inline("posts", "CREATE TABLE posts (id TEXT)")
        .with_required_columns(vec!["id".to_string(), "title".to_string()]);
    assert_eq!(schema.required_columns.len(), 2);
    assert_eq!(schema.required_columns[0], "id");
    assert_eq!(schema.required_columns[1], "title");
}

#[test]
fn schema_definition_serde_roundtrip_inline() {
    let schema = SchemaDefinition::inline("events", "CREATE TABLE events (id TEXT)")
        .with_required_columns(vec!["id".to_string()]);
    let json = serde_json::to_string(&schema).expect("serialize");
    let deserialized: SchemaDefinition = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.table, "events");
    assert_eq!(deserialized.required_columns, vec!["id"]);
}

#[test]
fn schema_definition_serde_roundtrip_file() {
    let schema = SchemaDefinition::file("items", "/path/items.sql");
    let json = serde_json::to_string(&schema).expect("serialize");
    let deserialized: SchemaDefinition = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.table, "items");
    assert!(matches!(deserialized.sql, SchemaSource::File(_)));
}

#[test]
fn schema_source_inline_variant() {
    let source = SchemaSource::Inline("SELECT 1".to_string());
    assert!(matches!(source, SchemaSource::Inline(ref s) if s == "SELECT 1"));
}

#[test]
fn schema_source_file_variant() {
    let source = SchemaSource::File(PathBuf::from("test.sql"));
    assert!(matches!(source, SchemaSource::File(ref p) if p.to_str().unwrap() == "test.sql"));
}

#[test]
fn seed_source_inline_variant() {
    let seed = SeedSource::Inline("INSERT INTO x VALUES (1)".to_string());
    assert!(matches!(seed, SeedSource::Inline(ref s) if s.contains("INSERT")));
}

#[test]
fn seed_source_file_variant() {
    let seed = SeedSource::File(PathBuf::from("seeds.sql"));
    assert!(matches!(seed, SeedSource::File(ref p) if p.to_str().unwrap() == "seeds.sql"));
}

#[test]
fn extension_role_new() {
    let role = ExtensionRole::new("admin", "Administrator", "Full access");
    assert_eq!(role.name, "admin");
    assert_eq!(role.display_name, "Administrator");
    assert_eq!(role.description, "Full access");
    assert!(role.permissions.is_empty());
}

#[test]
fn extension_role_with_permissions() {
    let role = ExtensionRole::new("editor", "Editor", "Can edit content")
        .with_permissions(vec!["read".to_string(), "write".to_string()]);
    assert_eq!(role.permissions.len(), 2);
    assert_eq!(role.permissions[0], "read");
    assert_eq!(role.permissions[1], "write");
}

#[test]
fn extension_role_serde_roundtrip() {
    let role = ExtensionRole::new("viewer", "Viewer", "Read-only access")
        .with_permissions(vec!["read".to_string()]);
    let json = serde_json::to_string(&role).expect("serialize");
    let deserialized: ExtensionRole = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.name, "viewer");
    assert_eq!(deserialized.display_name, "Viewer");
    assert_eq!(deserialized.permissions, vec!["read"]);
}

#[test]
fn extension_role_default_permissions_in_json() {
    let json = r#"{"name":"test","display_name":"Test","description":"Desc"}"#;
    let role: ExtensionRole = serde_json::from_str(json).expect("deserialize");
    assert!(role.permissions.is_empty());
}
