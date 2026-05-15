//! Unit tests for DatabaseExtension

use systemprompt_database::DatabaseExtension;
use systemprompt_extension::Extension;

#[test]
fn test_database_extension_metadata_id() {
    let ext = DatabaseExtension;
    let metadata = ext.metadata();
    assert_eq!(metadata.id, "database");
}

#[test]
fn test_database_extension_metadata_name() {
    let ext = DatabaseExtension;
    let metadata = ext.metadata();
    assert_eq!(metadata.name, "Database");
}

#[test]
fn test_database_extension_metadata_version() {
    let ext = DatabaseExtension;
    let metadata = ext.metadata();
    assert!(!metadata.version.is_empty());
}

#[test]
fn test_database_extension_dependencies() {
    let ext = DatabaseExtension;
    let deps = ext.dependencies();
    assert!(deps.is_empty());
}

#[test]
fn test_database_extension_schemas() {
    let ext = DatabaseExtension;
    let schemas = ext.schemas();
    assert!(!schemas.is_empty());
}

#[test]
fn test_database_extension_schemas_contains_functions() {
    let ext = DatabaseExtension;
    let schemas = ext.schemas();
    let has_functions = schemas.iter().any(|s| s.table == "functions");
    assert!(has_functions);
}

#[test]
fn test_database_extension_debug() {
    let ext = DatabaseExtension;
    let debug = format!("{:?}", ext);
    assert!(debug.contains("DatabaseExtension"));
}

#[test]
fn test_database_extension_clone() {
    let ext = DatabaseExtension;
    let cloned = ext;
    assert_eq!(ext.metadata().id, cloned.metadata().id);
}

#[test]
fn test_database_extension_default() {
    let ext = DatabaseExtension::default();
    assert_eq!(ext.metadata().id, "database");
}
