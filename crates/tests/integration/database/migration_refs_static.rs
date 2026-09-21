//! Static check: a migration may not name a trigger or view only a
//! declarative schema creates. Pure — no database.

use std::sync::Arc;

use systemprompt_database::check_migration_references;
use systemprompt_extension::{
    Extension, ExtensionMetadata, LoaderError, Migration, SchemaDefinition,
};

struct StubExtension {
    id: &'static str,
    schema: String,
    migrations: Vec<Migration>,
}

impl Extension for StubExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: self.id,
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(self.id, self.schema.clone())]
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }
}

fn ext(id: &'static str, schema: &str, migrations: Vec<Migration>) -> Arc<dyn Extension> {
    Arc::new(StubExtension {
        id,
        schema: schema.to_owned(),
        migrations,
    })
}

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS refs_t (id BIGINT PRIMARY KEY);\n\
                      CREATE OR REPLACE FUNCTION refs_fn() RETURNS trigger LANGUAGE plpgsql AS $$ \
                      BEGIN RETURN NEW; END $$;\n\
                      CREATE TRIGGER refs_trg BEFORE UPDATE ON refs_t FOR EACH ROW EXECUTE \
                      FUNCTION refs_fn();\n\
                      CREATE OR REPLACE VIEW refs_v AS SELECT id FROM refs_t;";

fn refused(migration_sql: &'static str) -> LoaderError {
    let e = ext(
        "refs",
        SCHEMA,
        vec![Migration::new(1, "probe", migration_sql)],
    );
    check_migration_references(&[e]).expect_err("must be refused")
}

fn accepted(migration_sql: &'static str) {
    let e = ext(
        "refs",
        SCHEMA,
        vec![Migration::new(1, "probe", migration_sql)],
    );
    check_migration_references(&[e]).expect("must be accepted");
}

fn assert_refusal(err: &LoaderError, kind: &str, object: &str) {
    match err {
        LoaderError::MigrationReferencesDeclarativeObject {
            extension,
            migration,
            kind: k,
            object: o,
            ..
        } => {
            assert_eq!(extension, "refs");
            assert_eq!(migration, "001_probe");
            assert_eq!(k, kind);
            assert_eq!(o, object);
        },
        other => panic!("expected MigrationReferencesDeclarativeObject, got {other:?}"),
    }
}

#[test]
fn disabling_a_declarative_only_trigger_is_refused() {
    let err = refused("ALTER TABLE refs_t DISABLE TRIGGER refs_trg;");
    assert_refusal(&err, "trigger", "refs_trg");
}

#[test]
fn dropping_a_declarative_only_trigger_without_if_exists_is_refused() {
    let err = refused("DROP TRIGGER refs_trg ON refs_t;");
    assert_refusal(&err, "trigger", "refs_trg");
}

#[test]
fn dropping_with_if_exists_is_accepted() {
    accepted("DROP TRIGGER IF EXISTS refs_trg ON refs_t;");
}

#[test]
fn querying_a_declarative_only_view_is_refused() {
    let err = refused("UPDATE refs_t SET id = id WHERE id IN (SELECT id FROM refs_v);");
    assert_refusal(&err, "view", "refs_v");
}

#[test]
fn a_view_the_same_migration_creates_is_accepted() {
    accepted(
        "CREATE OR REPLACE VIEW refs_v AS SELECT id FROM refs_t;\n\
         UPDATE refs_t SET id = id WHERE id IN (SELECT id FROM refs_v);",
    );
}

#[test]
fn a_trigger_some_migration_creates_is_accepted() {
    let creator = ext(
        "creator",
        "CREATE TABLE IF NOT EXISTS other_t (id BIGINT PRIMARY KEY);",
        vec![Migration::new(
            1,
            "create",
            "CREATE TRIGGER refs_trg BEFORE UPDATE ON refs_t FOR EACH ROW EXECUTE FUNCTION \
             refs_fn();",
        )],
    );
    let user = ext(
        "refs",
        SCHEMA,
        vec![Migration::new(
            1,
            "probe",
            "ALTER TABLE refs_t DISABLE TRIGGER refs_trg;",
        )],
    );
    check_migration_references(&[creator, user]).expect("a migration-created trigger is fair game");
}

#[test]
fn a_reference_inside_a_do_block_is_accepted() {
    accepted(
        "DO $$ BEGIN IF EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'refs_trg') THEN EXECUTE \
         'ALTER TABLE refs_t DISABLE TRIGGER refs_trg'; END IF; END $$;",
    );
}

#[test]
fn a_tombstoned_migration_is_not_scanned() {
    let e = ext("refs", SCHEMA, vec![Migration::tombstone(1, "probe")]);
    check_migration_references(&[e]).expect("tombstones carry no SQL that runs");
}

#[test]
fn ordinary_table_queries_are_not_mistaken_for_views() {
    accepted("UPDATE refs_t SET id = id WHERE id IN (SELECT id FROM refs_t);");
}
