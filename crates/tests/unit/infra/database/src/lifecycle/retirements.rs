//! DB tests for the retire phase: retired triggers, functions and views are
//! dropped ahead of every migration, idempotently, and any retirement that is
//! not a `DROP … IF EXISTS` of those kinds refuses the boot before anything
//! is written.

use std::sync::Arc;

use systemprompt_database::{DatabaseProvider, PostgresProvider};
use systemprompt_extension::{
    Extension, ExtensionMetadata, ExtensionRegistry, LoaderError, SchemaDefinition,
};
use systemprompt_test_fixtures::install_extension_schemas_with_config;

use super::installation::{drop_table, provider_and_db, unique_id};

struct RetiringExtension {
    id: &'static str,
    table: &'static str,
    retirements: Vec<String>,
}

impl Extension for RetiringExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: self.id,
            version: "0.0.1",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            self.table,
            format!(
                "CREATE TABLE IF NOT EXISTS \"{}\" (id BIGINT PRIMARY KEY);",
                self.table
            ),
        )]
    }

    fn retirements(&self) -> Vec<SchemaDefinition> {
        self.retirements
            .iter()
            .map(|sql| SchemaDefinition::sql_only(sql.clone()))
            .collect()
    }
}

fn registry(extensions: Vec<RetiringExtension>) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    for extension in extensions {
        registry
            .register(Arc::new(extension))
            .expect("register retiring extension");
    }
    registry
}

async fn exists(provider: &PostgresProvider, probe: &str) -> bool {
    let rows = provider
        .fetch_all(&format!("SELECT ({probe}) AS present"), &[])
        .await
        .expect("existence probe");
    rows.first()
        .and_then(|row| row["present"].as_bool())
        .unwrap_or(false)
}

async fn legacy_objects(provider: &PostgresProvider, prefix: &str) {
    provider
        .execute_batch(&format!(
            "CREATE TABLE {prefix}_legacy (id BIGINT PRIMARY KEY);
             CREATE FUNCTION {prefix}_fn() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RETURN NEW; END $$;
             CREATE TRIGGER {prefix}_trg AFTER INSERT ON {prefix}_legacy
                 FOR EACH ROW EXECUTE FUNCTION {prefix}_fn();
             CREATE VIEW {prefix}_view AS SELECT id FROM {prefix}_legacy;"
        ))
        .await
        .expect("legacy objects");
}

fn retire_all(prefix: &str) -> Vec<String> {
    vec![format!(
        "DROP VIEW IF EXISTS {prefix}_view;\nDROP TRIGGER IF EXISTS {prefix}_trg ON \
         {prefix}_legacy;\nDROP FUNCTION IF EXISTS {prefix}_fn();\nDROP PROCEDURE IF EXISTS \
         {prefix}_proc();"
    )]
}

#[tokio::test]
async fn retirements_drop_their_objects_and_are_a_no_op_on_the_next_boot() {
    let (provider, db) = provider_and_db().await;
    let prefix = unique_id("ret_ok");
    let table = unique_id("ret_ok_table");
    legacy_objects(&provider, prefix).await;
    let build = || RetiringExtension {
        id: unique_id("ret_ext"),
        table,
        retirements: retire_all(prefix),
    };

    for _ in 0..2 {
        install_extension_schemas_with_config(&registry(vec![build()]), &provider, &[])
            .await
            .expect("retirements applied");
    }

    assert!(
        !exists(
            &provider,
            &format!("to_regclass('{prefix}_view') IS NOT NULL")
        )
        .await
    );
    assert!(
        !exists(
            &provider,
            &format!("EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = '{prefix}_trg')")
        )
        .await
    );
    assert!(
        !exists(
            &provider,
            &format!("EXISTS (SELECT 1 FROM pg_proc WHERE proname = '{prefix}_fn')")
        )
        .await
    );
    assert!(
        exists(
            &provider,
            &format!("to_regclass('{prefix}_legacy') IS NOT NULL")
        )
        .await,
        "tables are never retired"
    );
    drop_table(&db, &format!("{prefix}_legacy")).await;
    drop_table(&db, table).await;
}

async fn refused(statement: &str) -> (LoaderError, bool) {
    let (provider, db) = provider_and_db().await;
    let prefix = unique_id("ret_refused");
    legacy_objects(&provider, prefix).await;
    let valid = RetiringExtension {
        id: unique_id("ret_valid"),
        table: unique_id("ret_valid_table"),
        retirements: retire_all(prefix),
    };
    let invalid = RetiringExtension {
        id: unique_id("ret_invalid"),
        table: unique_id("ret_invalid_table"),
        retirements: vec![statement.replace("{prefix}", prefix)],
    };

    let error =
        install_extension_schemas_with_config(&registry(vec![valid, invalid]), &provider, &[])
            .await
            .expect_err("retirement refused");
    let untouched = exists(
        &provider,
        &format!("to_regclass('{prefix}_view') IS NOT NULL"),
    )
    .await;
    drop_table(&db, &format!("{prefix}_legacy")).await;
    (error, untouched)
}

#[tokio::test]
async fn a_retired_table_is_refused_before_any_retirement_runs() {
    let (error, untouched) = refused("DROP TABLE IF EXISTS {prefix}_legacy;").await;

    assert!(
        error
            .to_string()
            .contains("only triggers, functions, procedures and views can be retired"),
        "{error}"
    );
    assert!(
        untouched,
        "the other extension's valid retirement did not run"
    );
}

#[tokio::test]
async fn a_retirement_without_if_exists_is_refused() {
    let (error, untouched) = refused("DROP VIEW {prefix}_view;").await;

    assert!(error.to_string().contains("must say IF EXISTS"), "{error}");
    assert!(untouched);
}

#[tokio::test]
async fn a_retirement_that_is_not_a_drop_is_refused() {
    let (error, untouched) = refused("DELETE FROM {prefix}_legacy;").await;

    assert!(error.to_string().contains("only DROP"), "{error}");
    assert!(untouched);
}

#[tokio::test]
async fn an_unparseable_retirement_is_refused() {
    let (error, untouched) = refused("DROP VIEW IF EXISTS ((( {prefix};").await;

    assert!(
        matches!(error, LoaderError::SchemaInstallationStepFailed { .. }),
        "{error}"
    );
    assert!(error.to_string().contains("retirement: SQL"), "{error}");
    assert!(untouched);
}
