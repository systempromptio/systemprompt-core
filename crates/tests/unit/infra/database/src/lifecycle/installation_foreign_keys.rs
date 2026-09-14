//! DB-backed tests for the deferred-foreign-key phase of
//! `install_extension_schemas*`: a key declared inline on a declarative
//! `CREATE TABLE` is applied after migrations, once per database, and never
//! turns pre-existing rows into a boot failure.

use systemprompt_database::install_extension_schemas_with_config;
use systemprompt_extension::{Migration, SchemaDefinition};

use super::installation::{
    StubExtension, drop_table, leak, provider_and_db_or_skip, registry_with, table_exists,
    unique_id,
};

async fn foreign_key_count(db: &systemprompt_database::DbPool, table: &str) -> i64 {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_constraint WHERE contype = 'f' AND conrelid = to_regclass($1)",
    )
    .bind(format!("\"{table}\""))
    .fetch_one(&*pg)
    .await
    .expect("constraint count")
}

async fn foreign_key_validated(db: &systemprompt_database::DbPool, table: &str) -> Option<bool> {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query_scalar(
        "SELECT convalidated FROM pg_constraint WHERE contype = 'f' AND conrelid = to_regclass($1)",
    )
    .bind(format!("\"{table}\""))
    .fetch_optional(&*pg)
    .await
    .expect("constraint probe")
}

async fn run_sql(db: &systemprompt_database::DbPool, sql: String) {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(&*pg)
        .await
        .expect("fixture sql");
}

async fn forget_migrations(db: &systemprompt_database::DbPool, ext_id: &str) {
    let pg = db.write_pool_arc().expect("write pool");
    let _ = sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&*pg)
        .await;
}

/// The 2026-09-14 incident: `parent` already exists on the database without
/// the composite unique index, the declarative schema declares it with a
/// `UNIQUE(user_id, id)` and a `child` whose composite key references it, and
/// migration 001 is what gives the existing database that index. An inline
/// key failed in the structural phase; the deferred key waits for 001.
#[tokio::test]
async fn legacy_table_without_referenced_unique_gets_fk_after_migration_adds_index() {
    let Some((provider, db)) = provider_and_db_or_skip().await else {
        return;
    };
    let parent = unique_id("fk_legacy_parent");
    let child = unique_id("fk_legacy_child");
    let ext_id = unique_id("fk_legacy_ext");

    run_sql(
        &db,
        format!("CREATE TABLE \"{parent}\" (id TEXT PRIMARY KEY, user_id TEXT NOT NULL)"),
    )
    .await;

    let schema_sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{parent}\" (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, \
         UNIQUE(user_id, id));\n\
         CREATE TABLE IF NOT EXISTS \"{child}\" (id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, \
         parent_id TEXT NOT NULL, FOREIGN KEY (owner_id, parent_id) REFERENCES \"{parent}\" \
         (user_id, id));"
    );
    let migration_sql = leak(format!(
        "CREATE UNIQUE INDEX IF NOT EXISTS \"{parent}_owner_id\" ON \"{parent}\" (user_id, id)"
    ));
    let registry = registry_with(StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::sql_only(schema_sql)],
        seeds: vec![],
        migrations: vec![Migration::new(1, "owner_index", migration_sql)],
    });

    install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect("a legacy database converges: table, migration, then the deferred key");

    assert!(table_exists(&db, child).await);
    assert_eq!(foreign_key_count(&db, child).await, 1);
    assert_eq!(foreign_key_validated(&db, child).await, Some(true));

    drop_table(&db, child).await;
    drop_table(&db, parent).await;
    forget_migrations(&db, ext_id).await;
}

#[tokio::test]
async fn installing_twice_adds_exactly_one_foreign_key() {
    let Some((provider, db)) = provider_and_db_or_skip().await else {
        return;
    };
    let parent = unique_id("fk_twice_parent");
    let child = unique_id("fk_twice_child");
    let ext_id = unique_id("fk_twice_ext");
    let schema_sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{parent}\" (id TEXT PRIMARY KEY);\n\
         CREATE TABLE IF NOT EXISTS \"{child}\" (id TEXT PRIMARY KEY, parent_id TEXT NOT NULL \
         REFERENCES \"{parent}\" (id) ON DELETE CASCADE);"
    );
    let build = || StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::sql_only(schema_sql.clone())],
        seeds: vec![],
        migrations: vec![],
    };

    for _ in 0..2 {
        install_extension_schemas_with_config(&registry_with(build()), &provider, &[])
            .await
            .expect("install");
    }
    assert_eq!(foreign_key_count(&db, child).await, 1);

    drop_table(&db, child).await;
    drop_table(&db, parent).await;
}

/// A key a migration created under its own name counts: the installer matches
/// on the constrained and referenced columns, never on the name.
#[tokio::test]
async fn a_migration_authored_key_under_another_name_is_not_duplicated() {
    let Some((provider, db)) = provider_and_db_or_skip().await else {
        return;
    };
    let parent = unique_id("fk_named_parent");
    let child = unique_id("fk_named_child");
    let ext_id = unique_id("fk_named_ext");

    run_sql(
        &db,
        format!(
            "CREATE TABLE \"{parent}\" (id TEXT PRIMARY KEY);\n\
             CREATE TABLE \"{child}\" (id TEXT PRIMARY KEY, parent_id TEXT NOT NULL, CONSTRAINT \
             \"{child}_hand_made\" FOREIGN KEY (parent_id) REFERENCES \"{parent}\" (id));"
        ),
    )
    .await;

    let schema_sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{parent}\" (id TEXT PRIMARY KEY);\n\
         CREATE TABLE IF NOT EXISTS \"{child}\" (id TEXT PRIMARY KEY, parent_id TEXT NOT NULL \
         REFERENCES \"{parent}\" (id));"
    );
    let registry = registry_with(StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::sql_only(schema_sql)],
        seeds: vec![],
        migrations: vec![],
    });
    install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect("install");

    assert_eq!(foreign_key_count(&db, child).await, 1);

    drop_table(&db, child).await;
    drop_table(&db, parent).await;
}

/// Rows that already violate the key must not turn a boot into an outage:
/// the key is added `NOT VALID` — enforced for new rows — and installation
/// succeeds.
#[tokio::test]
async fn orphan_rows_leave_the_key_not_valid_and_install_succeeds() {
    let Some((provider, db)) = provider_and_db_or_skip().await else {
        return;
    };
    let parent = unique_id("fk_orphan_parent");
    let child = unique_id("fk_orphan_child");
    let ext_id = unique_id("fk_orphan_ext");

    run_sql(
        &db,
        format!(
            "CREATE TABLE \"{parent}\" (id TEXT PRIMARY KEY);\n\
             CREATE TABLE \"{child}\" (id TEXT PRIMARY KEY, parent_id TEXT NOT NULL);\n\
             INSERT INTO \"{child}\" VALUES ('c1', 'no-such-parent');"
        ),
    )
    .await;

    let schema_sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{parent}\" (id TEXT PRIMARY KEY);\n\
         CREATE TABLE IF NOT EXISTS \"{child}\" (id TEXT PRIMARY KEY, parent_id TEXT NOT NULL \
         REFERENCES \"{parent}\" (id));"
    );
    let registry = registry_with(StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::sql_only(schema_sql)],
        seeds: vec![],
        migrations: vec![],
    });
    install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect("orphan rows must not fail installation");

    assert_eq!(foreign_key_validated(&db, child).await, Some(false));

    let pg = db.write_pool_arc().expect("write pool");
    let new_orphan = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO \"{child}\" VALUES ('c2', 'still-no-parent')"
    )))
    .execute(&*pg)
    .await;
    assert!(new_orphan.is_err(), "a NOT VALID key still guards new rows");

    drop_table(&db, child).await;
    drop_table(&db, parent).await;
}

#[tokio::test]
async fn a_deferred_foreign_key_that_cannot_be_satisfied_names_the_source_table() {
    let Some((provider, db)) = provider_and_db_or_skip().await else {
        return;
    };
    let parent = unique_id("fk_unsat_parent");
    let child = unique_id("fk_unsat_child");
    let ext_id = unique_id("fk_unsat_ext");

    // The linter would refuse a same-extension reference without a declared
    // unique, so the referenced table comes from "another extension": it is
    // pre-created without the composite unique and not declared here.
    run_sql(
        &db,
        format!("CREATE TABLE \"{parent}\" (id TEXT PRIMARY KEY, user_id TEXT NOT NULL)"),
    )
    .await;
    let schema_sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{child}\" (id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, \
         parent_id TEXT NOT NULL, FOREIGN KEY (owner_id, parent_id) REFERENCES \"{parent}\" \
         (user_id, id));"
    );
    let registry = registry_with(StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::sql_only(schema_sql)],
        seeds: vec![],
        migrations: vec![],
    });
    let err = install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect_err("no unique index on the referenced columns");
    let message = err.to_string();
    assert!(
        message.contains("declared inline on CREATE TABLE") && message.contains(child),
        "{message}"
    );
    assert!(message.contains("(user_id, id)"), "{message}");
    assert!(
        table_exists(&db, child).await,
        "the structural phase committed before the key phase failed"
    );
    assert_eq!(foreign_key_count(&db, child).await, 0);

    drop_table(&db, child).await;
    drop_table(&db, parent).await;
}
