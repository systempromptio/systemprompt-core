use std::any::TypeId;
use std::sync::Arc;

use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::error::LoaderError;
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, ExtensionRole, Migration,
    SchemaDefinition,
};
use systemprompt_identifiers::ExtensionId;

fn contributions(ext: &dyn Extension) -> ContributionSink {
    let mut sink = ContributionSink::default();
    ext.contribute(&mut Contributions::__new(
        ExtensionId::new(ext.metadata().id),
        &mut sink,
    ))
    .expect("the member contributes");
    assert!(sink.take_errors().is_empty());
    sink
}

struct MinimalExt;

impl Extension for MinimalExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "minimal",
            name: "Minimal",
            version: "0.1.0",
        }
    }

    fn contribute(&self, _c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        Ok(())
    }
}

struct SchemaExt;

impl Extension for SchemaExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "schema-bearing",
            name: "Schema Bearing",
            version: "1.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.schema(SchemaDefinition::new(
            "things",
            "CREATE TABLE things (id TEXT)",
        ))
        .migrations(vec![Migration::new(1, "seed_things", "SELECT 1;")])
        .cross_table("users");
        Ok(())
    }
}

struct DepExt;

impl Extension for DepExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "dep-ext",
            name: "With Deps",
            version: "1.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.depends_on::<MinimalExt>();
        Ok(())
    }
}

struct RolesExt;

impl Extension for RolesExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "roles-ext",
            name: "Roles",
            version: "1.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.role(ExtensionRole::new("viewer", "Viewer", "Read-only"));
        Ok(())
    }
}

#[test]
fn metadata_carries_id_name_and_version() {
    let metadata = MinimalExt.metadata();
    assert_eq!(metadata.id, "minimal");
    assert_eq!(metadata.name, "Minimal");
    assert_eq!(metadata.version, "0.1.0");
}

#[test]
fn a_member_that_contributes_nothing_leaves_an_empty_sink() {
    let sink = contributions(&MinimalExt);
    assert!(sink.schemas().is_empty());
    assert!(sink.migrations().is_empty());
    assert!(sink.seeds().is_empty());
    assert!(sink.cross_tables().is_empty());
    assert!(sink.roles().is_empty());
    assert!(sink.depends_on().is_empty());
    assert!(sink.assets().is_empty());
    assert!(sink.site_auth().is_empty());
}

#[test]
fn contributed_schemas_migrations_and_cross_tables_are_recorded() {
    let sink = contributions(&SchemaExt);
    assert_eq!(sink.schemas().len(), 1);
    assert_eq!(sink.schemas()[0].table.as_deref(), Some("things"));
    assert_eq!(sink.migrations().len(), 1);
    assert_eq!(sink.migrations()[0].version, 1);
    assert_eq!(sink.cross_tables(), ["users"]);
}

#[test]
fn a_member_dependency_is_recorded_by_type() {
    let sink = contributions(&DepExt);
    assert_eq!(sink.depends_on().len(), 1);
    assert_eq!(sink.depends_on()[0].type_id(), TypeId::of::<MinimalExt>());
}

#[test]
fn contributed_roles_are_recorded() {
    let sink = contributions(&RolesExt);
    assert_eq!(sink.roles().len(), 1);
    assert_eq!(sink.roles()[0].name, "viewer");
}

#[test]
fn extension_is_arc_dyn_compatible() {
    let ext: Arc<dyn Extension> = Arc::new(MinimalExt);
    assert_eq!(ext.metadata().id, "minimal");
    assert!(contributions(ext.as_ref()).schemas().is_empty());
}

#[test]
fn loader_error_migration_not_reversible_display() {
    let err = LoaderError::MigrationNotReversible {
        extension: ExtensionId::new("my-ext"),
        version: 5,
    };
    let msg = err.to_string();
    assert!(msg.contains("my-ext"));
    assert!(msg.contains("5"));
    assert!(msg.contains("not reversible"));
}

#[test]
fn loader_error_dependency_cycle_display() {
    let err = LoaderError::DependencyCycle {
        chain: "ext-a -> ext-b -> ext-a".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("ext-a -> ext-b -> ext-a"));
    assert!(msg.contains("cycle"));
}

#[test]
fn loader_error_cross_extension_alter_undeclared_display() {
    let err = LoaderError::CrossExtensionAlterUndeclared {
        extension: ExtensionId::new("plugin-x"),
        table: "users".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("plugin-x"));
    assert!(msg.contains("users"));
    assert!(msg.contains("cross-extension"));
}

#[test]
fn loader_error_duplicate_table_owner_display() {
    let err = LoaderError::DuplicateTableOwner {
        table: "orders".to_string(),
        extension_a: ExtensionId::new("billing"),
        extension_b: ExtensionId::new("shop"),
    };
    let msg = err.to_string();
    assert!(msg.contains("orders"));
    assert!(msg.contains("billing"));
    assert!(msg.contains("shop"));
}

#[test]
fn loader_error_cross_extension_table_not_owned_display() {
    let err = LoaderError::CrossExtensionTableNotOwned {
        extension: ExtensionId::new("addon"),
        table: "nonexistent_table".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("addon"));
    assert!(msg.contains("nonexistent_table"));
}

#[test]
fn loader_error_invalid_seed_statement_display() {
    let err = LoaderError::InvalidSeedStatement {
        extension: ExtensionId::new("seeder"),
        seed: "initial_data".to_string(),
        statement: "CREATE TABLE".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("seeder"));
    assert!(msg.contains("initial_data"));
    assert!(msg.contains("CREATE TABLE"));
}

#[test]
fn loader_error_seed_insert_not_idempotent_display() {
    let err = LoaderError::SeedInsertNotIdempotent {
        extension: ExtensionId::new("ext"),
        seed: "seed_roles".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("ext"));
    assert!(msg.contains("seed_roles"));
    assert!(msg.contains("idempotent"));
}

#[test]
fn loader_error_seed_failed_display() {
    let err = LoaderError::SeedFailed {
        extension: ExtensionId::new("data-ext"),
        seed: "load_defaults".to_string(),
        context: "execute".to_string(),
        source: "connection timeout".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("data-ext"));
    assert!(msg.contains("load_defaults"));
    assert!(msg.contains("connection timeout"));
}

#[test]
fn schema_definition_with_schema_sets_schema_name() {
    let schema =
        SchemaDefinition::new("events", "CREATE TABLE events (id TEXT)").with_schema("audit");
    assert_eq!(schema.schema_name(), "audit");
}

#[test]
fn schema_definition_schema_name_defaults_to_public() {
    let schema = SchemaDefinition::new("events", "CREATE TABLE events (id TEXT)");
    assert_eq!(schema.schema_name(), "public");
}

#[test]
fn schema_definition_with_schema_roundtrip_serde() {
    let schema = SchemaDefinition::new("logs", "CREATE TABLE logs (id TEXT)").with_schema("audit");
    let json = serde_json::to_string(&schema).expect("serialize");
    let back: SchemaDefinition = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.schema_name(), "audit");
}
