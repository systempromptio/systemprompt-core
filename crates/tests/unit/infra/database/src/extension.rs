//! Unit tests for DatabaseExtension

use systemprompt_database::DatabaseExtension;
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Extension, StandingTag};
use systemprompt_test_fixtures::admitted_member;

fn contributions() -> ContributionSink {
    admitted_member(&DatabaseExtension, StandingTag::Core)
        .expect("database contributes")
        .sink
}

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
    assert!(contributions().depends_on().is_empty());
}

#[test]
fn test_database_extension_schemas() {
    let sink = contributions();
    let schemas = sink.schemas();
    assert_eq!(schemas.len(), 17);
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("schema_table_owners"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("schema_purge_records"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("schema_install_fingerprint"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("schema_trigger_digests"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("config_plane_applied"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("services"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("projection_backlog"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("extension_migrations"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("trace_facts"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("extension_migration_compat"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("instance_leases"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("instance_endpoints"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("release_stamps"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.as_deref() == Some("config_version"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.is_none() && s.sql.contains("update_timestamp_trigger"))
    );
    assert!(
        schemas
            .iter()
            .any(|s| s.table.is_none() && s.sql.contains("sp_bump_config_version"))
    );
}

#[test]
fn test_database_extension_declares_shared_functions_without_a_table() {
    let sink = contributions();
    let schemas = sink.schemas();

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
fn test_database_extension_default() {
    let ext = DatabaseExtension::default();
    assert_eq!(ext.metadata().id, "database");
}
