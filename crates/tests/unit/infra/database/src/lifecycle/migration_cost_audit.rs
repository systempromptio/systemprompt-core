use std::sync::Arc;

use systemprompt_database::{audit_migration_cost, audit_one};
use systemprompt_extension::{Extension, ExtensionMetadata, Migration, SchemaDefinition};
use systemprompt_identifiers::ExtensionId;

struct Ext {
    id: &'static str,
    migrations: Vec<Migration>,
}

impl Extension for Ext {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "cost",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }
}

const HOT: &[&str] = &["hot_rows"];

#[test]
fn the_catalogue_audit_reports_only_hot_rewrites_and_skips_tombstones() {
    let extensions: Vec<Arc<dyn Extension>> = vec![
        Arc::new(Ext {
            id: "cost_a",
            migrations: vec![
                Migration::new(1, "create", "CREATE TABLE hot_rows (id BIGINT);"),
                Migration::new(2, "backfill", "UPDATE hot_rows SET id = id + 1;"),
                Migration::tombstone(3, "gone"),
            ],
        }),
        Arc::new(Ext {
            id: "cost_b",
            migrations: vec![Migration::new(
                7,
                "purge",
                "DELETE FROM hot_rows WHERE id < 0; DELETE FROM cold_rows;",
            )],
        }),
    ];
    let found = audit_migration_cost(&extensions, HOT);
    let labels: Vec<String> = found.iter().map(|c| c.label()).collect();
    assert_eq!(labels, ["cost_a/002_backfill", "cost_b/007_purge"]);
    assert!(found.iter().all(|c| c.is_undeclared()));
    assert_eq!(
        found[0].statement_summary(),
        "statement 1 UPDATE on hot_rows"
    );
    assert_eq!(found[1].statements.len(), 1);
    assert_eq!(found[1].statements[0].form, "DELETE from");
}

#[test]
fn an_empty_catalogue_reports_nothing() {
    assert!(audit_migration_cost(&[], HOT).is_empty());
}

#[test]
fn a_malformed_cost_directive_is_reported_even_without_hot_writes() {
    let cost = audit_one(
        &ExtensionId::new("ext"),
        "004_x",
        "-- @cost: rows=lots\nCREATE TABLE cold (id BIGINT);",
        HOT,
    )
    .expect("reported");
    assert!(cost.malformed.is_some());
    assert!(cost.statements.is_empty());
    assert!(!cost.is_undeclared());
}

#[test]
fn unparseable_sql_yields_no_statements() {
    assert!(
        audit_one(
            &ExtensionId::new("ext"),
            "005_x",
            "UPDATE hot_rows SET",
            HOT
        )
        .is_none()
    );
}

#[test]
fn literal_inserts_are_not_backfills_but_select_inserts_are() {
    assert!(
        audit_one(
            &ExtensionId::new("ext"),
            "006_x",
            "INSERT INTO hot_rows (id) VALUES (1);",
            HOT
        )
        .is_none()
    );
    let cost = audit_one(
        &ExtensionId::new("ext"),
        "007_x",
        "INSERT INTO hot_rows (id) SELECT id FROM cold;",
        HOT,
    )
    .expect("flagged");
    assert_eq!(cost.statements[0].form, "INSERT … SELECT into");
    assert!(
        audit_one(
            &ExtensionId::new("ext"),
            "008_x",
            "CREATE INDEX CONCURRENTLY hot_rows_id ON hot_rows (id);",
            HOT
        )
        .is_none()
    );
}
