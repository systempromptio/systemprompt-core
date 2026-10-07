//! `schema_currency`: the read-only check a node runs when it boots without
//! migrating. An extension whose tables were never created is reported fresh,
//! a defined migration without a ledger row is pending, an applied migration
//! whose file changed is drift, and an installed database is current.

use systemprompt_database::schema_currency;
use systemprompt_extension::{Migration, SchemaDefinition};

use super::installation::{
    StubExtension, drop_table, leak, provider_and_db, registry_with, unique_id,
};
use systemprompt_test_fixtures::install_extension_schemas_with_config;

fn stub(ext_id: &'static str, table: &'static str, migration_sql: &'static str) -> StubExtension {
    StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY, note TEXT);"),
        )],
        seeds: vec![],
        migrations: vec![Migration::new(1, "add_note", migration_sql)],
    }
}

fn migration_sql(table: &str) -> &'static str {
    leak(format!(
        "ALTER TABLE \"{table}\" ADD COLUMN IF NOT EXISTS note TEXT;"
    ))
}

#[tokio::test]
async fn an_uninstalled_extension_is_reported_fresh() {
    let (provider, _db) = provider_and_db().await;
    let ext_id = unique_id("currency_fresh");
    let table = unique_id("currency_fresh_t");
    let registry = registry_with(stub(ext_id, table, migration_sql(table)));

    let currency = schema_currency(&provider, &registry, &[])
        .await
        .expect("currency");

    assert!(!currency.is_current());
    assert_eq!(currency.fresh_extensions.len(), 1);
    assert_eq!(currency.fresh_extensions[0].as_str(), ext_id);
}

#[tokio::test]
async fn an_installed_extension_is_current_until_a_ledger_row_goes_missing() {
    let (provider, db) = provider_and_db().await;
    let ext_id = unique_id("currency_pending");
    let table = unique_id("currency_pending_t");
    let sql = migration_sql(table);

    install_extension_schemas_with_config(&registry_with(stub(ext_id, table, sql)), &provider, &[])
        .await
        .expect("install");
    let currency = schema_currency(&provider, &registry_with(stub(ext_id, table, sql)), &[])
        .await
        .expect("currency");
    assert!(currency.is_current(), "{currency:?}");

    sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1 AND version = 1")
        .bind(ext_id)
        .execute(&*db.write_pool())
        .await
        .expect("drop ledger row");

    let currency = schema_currency(&provider, &registry_with(stub(ext_id, table, sql)), &[])
        .await
        .expect("currency");
    assert!(!currency.is_current());
    assert!(currency.fresh_extensions.is_empty());
    assert_eq!(currency.pending.len(), 1, "{currency:?}");
    assert_eq!(currency.pending[0].version, 1);
    assert_eq!(currency.pending[0].name, "add_note");

    sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&*db.write_pool())
        .await
        .expect("drop ledger rows");
    drop_table(&db, table).await;
}

#[tokio::test]
async fn an_edited_applied_migration_is_reported_as_drift() {
    let (provider, db) = provider_and_db().await;
    let ext_id = unique_id("currency_drift");
    let table = unique_id("currency_drift_t");

    install_extension_schemas_with_config(
        &registry_with(stub(ext_id, table, migration_sql(table))),
        &provider,
        &[],
    )
    .await
    .expect("install");

    let edited = leak(format!(
        "ALTER TABLE \"{table}\" ADD COLUMN IF NOT EXISTS note VARCHAR(64);"
    ));
    let currency = schema_currency(&provider, &registry_with(stub(ext_id, table, edited)), &[])
        .await
        .expect("currency");
    assert!(!currency.is_current());
    assert!(currency.pending.is_empty());
    assert_eq!(currency.drift.len(), 1, "{currency:?}");

    sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&*db.write_pool())
        .await
        .expect("drop ledger rows");
    drop_table(&db, table).await;
}
