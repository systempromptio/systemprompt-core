use std::sync::Arc;

use systemprompt_database::check_migration_references;
use systemprompt_extension::{
    Extension, ExtensionMetadata, LoaderError, Migration, SchemaDefinition,
};

struct Ext {
    schema: &'static str,
    migrations: Vec<Migration>,
}

impl Extension for Ext {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "refs_errors",
            name: "refs",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new("refs_t", self.schema)]
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }
}

const SCHEMA: &str = "CREATE TABLE refs_t (id BIGINT); CREATE VIEW refs_v AS SELECT id FROM \
                      refs_t;";

fn check(schema: &'static str, migrations: Vec<Migration>) -> Result<(), LoaderError> {
    let extensions: Vec<Arc<dyn Extension>> = vec![Arc::new(Ext { schema, migrations })];
    check_migration_references(&extensions)
}

#[test]
fn an_unparseable_migration_is_refused_with_its_label() {
    let err = check(SCHEMA, vec![Migration::new(4, "broken", "SELEC 1;")]).expect_err("parse");
    match err {
        LoaderError::SchemaInstallationStepFailed { context, .. } => {
            assert!(context.contains("004_broken"), "{context}");
            assert!(context.contains("SQL parse failed"), "{context}");
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn an_unparseable_declarative_schema_is_refused() {
    let err = check("CREATE TABL nope (id int);", vec![]).expect_err("parse");
    assert!(
        matches!(err, LoaderError::SchemaInstallationStepFailed { ref context, .. } if context == "SQL parse failed"),
        "{err:?}"
    );
}

#[test]
fn the_first_of_several_violations_is_reported() {
    let err = check(
        SCHEMA,
        vec![
            Migration::new(1, "first", "ALTER TABLE refs_t DISABLE TRIGGER audit_one;"),
            Migration::new(2, "second", "ALTER TABLE refs_t ENABLE TRIGGER audit_two;"),
            Migration::new(
                3,
                "third",
                "UPDATE refs_t SET id = id WHERE id IN (SELECT id FROM refs_v);",
            ),
        ],
    )
    .expect_err("violations");
    match err {
        LoaderError::MigrationTogglesTriggerByName {
            migration, trigger, ..
        } => {
            assert_eq!(migration, "001_first");
            assert_eq!(trigger, "audit_one");
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn several_declarative_references_report_the_first() {
    let err = check(
        SCHEMA,
        vec![
            Migration::new(
                5,
                "reads",
                "UPDATE refs_t SET id = id WHERE id IN (SELECT id FROM refs_v);",
            ),
            Migration::new(
                6,
                "reads_again",
                "UPDATE refs_t SET id = id WHERE id IN (SELECT id FROM refs_v);",
            ),
        ],
    )
    .expect_err("violations");
    match err {
        LoaderError::MigrationReferencesDeclarativeObject {
            migration, object, ..
        } => {
            assert_eq!(migration, "005_reads");
            assert_eq!(object, "refs_v");
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_clean_catalogue_passes() {
    check(
        SCHEMA,
        vec![Migration::new(
            7,
            "plain",
            "ALTER TABLE refs_t ADD COLUMN note TEXT;",
        )],
    )
    .expect("no references to declarative objects");
}
