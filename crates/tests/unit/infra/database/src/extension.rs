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
    assert!(
        metadata.version.contains('.'),
        "version should be dotted semver: {}",
        metadata.version
    );
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
    assert_eq!(schemas.len(), 2);
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("extension_migrations"))
    );
}

#[test]
fn test_database_extension_declares_shared_functions_without_a_table() {
    let ext = DatabaseExtension;
    let schemas = ext.schemas();

    let functions = schemas
        .iter()
        .find(|s| s.sql.contains("update_timestamp_trigger"))
        .expect("the shared trigger function is declared");

    assert!(
        functions.table.is_none(),
        "the shared functions definition creates no table and must declare none"
    );
    assert!(!functions.sql.to_uppercase().contains("CREATE TABLE"));
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
