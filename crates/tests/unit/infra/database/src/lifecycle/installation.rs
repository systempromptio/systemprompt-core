//! DB-backed tests for `install_extension_schemas*`: schema installation
//! phases, seed application and linting, ownership validation, and the
//! disabled-extension skip path.

use std::sync::Arc;

use systemprompt_database::{DbPool, PostgresProvider, install_extension_schemas_with_config};
use systemprompt_extension::{
    Extension, ExtensionMetadata, ExtensionRegistry, LoaderError, SchemaDefinition, Seed,
};

use crate::services::db_helper::pool;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

struct StubExtension {
    id: &'static str,
    schemas: Vec<SchemaDefinition>,
    seeds: Vec<Seed>,
}

impl Extension for StubExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: self.id,
            version: "0.0.1",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        self.schemas.clone()
    }

    fn seeds(&self) -> Vec<Seed> {
        self.seeds.clone()
    }
}

fn unique_id(prefix: &str) -> &'static str {
    leak(format!("{prefix}_{}", uuid::Uuid::new_v4().simple()))
}

fn registry_with(ext: StubExtension) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register stub");
    registry
}

async fn provider_and_db() -> Option<(PostgresProvider, DbPool)> {
    let db = pool().await?;
    let pg = db.write_pool_arc().ok()?;
    Some((PostgresProvider::from_pool(pg), db))
}

async fn drop_table(db: &DbPool, table: &str) {
    let pg = db.write_pool_arc().expect("write pool");
    let ddl = format!("DROP TABLE IF EXISTS \"{table}\"");
    let _ = sqlx::query(sqlx::AssertSqlSafe(ddl)).execute(&*pg).await;
}

async fn table_exists(db: &DbPool, table: &str) -> bool {
    let pg = db.write_pool_arc().expect("write pool");
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND \
         table_name = $1)",
    )
    .bind(table)
    .fetch_one(&*pg)
    .await
    .expect("table existence probe")
}

#[tokio::test]
async fn install_creates_schema_index_and_applies_seed_idempotently() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_ok");
    let ext_id = unique_id("ext_ok");
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY, label TEXT);\nCREATE \
         INDEX IF NOT EXISTS \"{table}_label_idx\" ON \"{table}\" (label);"
    );
    let seed_sql = leak(format!(
        "INSERT INTO \"{table}\" (id, label) VALUES (1, 'seeded') ON CONFLICT (id) DO NOTHING;"
    ));

    let build = || StubExtension {
        id: ext_id,
        schemas: vec![
            SchemaDefinition::new(table, sql.clone())
                .with_required_columns(vec!["id".to_owned(), "label".to_owned()]),
        ],
        seeds: vec![Seed::new(unique_id("seed"), seed_sql)],
    };

    for _ in 0..2 {
        install_extension_schemas_with_config(&registry_with(build()), &provider, &[])
            .await
            .expect("install");
    }

    let pg = db.write_pool_arc().expect("write pool");
    let seeded: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM \"{table}\" WHERE label = 'seeded'"
    )))
    .fetch_one(&*pg)
    .await
    .expect("seed count");
    assert_eq!(seeded, 1);

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_skips_disabled_extensions() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_disabled");
    let ext_id = unique_id("ext_disabled");
    let ext = StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![],
    };

    install_extension_schemas_with_config(&registry_with(ext), &provider, &[ext_id.to_owned()])
        .await
        .expect("install with extension disabled");

    assert!(!table_exists(&db, table).await);
}

#[tokio::test]
async fn install_rejects_seed_with_delete_statement() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_seed_delete");
    let ext = StubExtension {
        id: unique_id("ext_seed_delete"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![Seed::new(
            unique_id("seed"),
            leak(format!("DELETE FROM \"{table}\";")),
        )],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("delete seed rejected");
    assert!(
        matches!(err, LoaderError::InvalidSeedStatement { statement, .. } if statement == "DELETE")
    );

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_rejects_non_idempotent_insert_seed() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_seed_plain");
    let ext = StubExtension {
        id: unique_id("ext_seed_plain"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![Seed::new(
            unique_id("seed"),
            leak(format!("INSERT INTO \"{table}\" (id) VALUES (1);")),
        )],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("plain insert seed rejected");
    assert!(matches!(err, LoaderError::SeedInsertNotIdempotent { .. }));

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_fails_when_required_column_is_missing() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_missing_col");
    let ext = StubExtension {
        id: unique_id("ext_missing_col"),
        schemas: vec![
            SchemaDefinition::new(
                table,
                format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
            )
            .with_required_columns(vec!["id".to_owned(), "phantom_column".to_owned()]),
        ],
        seeds: vec![],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("missing required column rejected");
    assert!(matches!(err, LoaderError::SchemaInstallationFailed { .. }));
    assert!(err.to_string().contains("phantom_column"));

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_rejects_duplicate_table_ownership() {
    let Some((provider, _db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_shared");
    let sql = format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);");
    let mut registry = ExtensionRegistry::new();
    for prefix in ["ext_owner_a", "ext_owner_b"] {
        registry
            .register(Arc::new(StubExtension {
                id: unique_id(prefix),
                schemas: vec![SchemaDefinition::new(table, sql.clone())],
                seeds: vec![],
            }))
            .expect("register stub");
    }

    let err = install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect_err("duplicate ownership rejected");
    assert!(matches!(err, LoaderError::DuplicateTableOwner { .. }));
}

#[tokio::test]
async fn install_rejects_imperative_sql_in_declarative_schema() {
    let Some((provider, _db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_imperative");
    let ext = StubExtension {
        id: unique_id("ext_imperative"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!(
                "CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);\nALTER TABLE \
                 \"{table}\" ADD COLUMN extra TEXT;"
            ),
        )],
        seeds: vec![],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("imperative DDL rejected");
    assert!(matches!(err, LoaderError::SchemaInstallationFailed { .. }));
    assert!(err.to_string().contains("Imperative SQL"));
}

async fn seed_rejection(seed_sql: &'static str) -> LoaderError {
    let (provider, _db) = provider_and_db().await.expect("db required");
    let table = unique_id("install_seed_kind");
    let ext = StubExtension {
        id: unique_id("ext_seed_kind"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![Seed::new(unique_id("seed"), seed_sql)],
    };
    install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("disallowed seed statement rejected")
}

#[tokio::test]
async fn install_rejects_seed_statements_by_classified_kind() {
    if provider_and_db().await.is_none() {
        return;
    }
    let cases: [(&'static str, &'static str); 7] = [
        ("SELECT 1;", "SELECT"),
        ("CREATE TABLE seed_smuggled_ddl (id BIGINT);", "CREATE"),
        ("CREATE INDEX seed_idx ON seed_t (id);", "CREATE"),
        ("ALTER TABLE seed_t ADD COLUMN x TEXT;", "ALTER"),
        ("DROP TABLE seed_t;", "DROP"),
        ("TRUNCATE seed_t;", "TRUNCATE"),
        ("GRANT SELECT ON seed_t TO PUBLIC;", "GRANT"),
    ];
    for (sql, expected_kind) in cases {
        let err = seed_rejection(sql).await;
        match err {
            LoaderError::InvalidSeedStatement { statement, .. } => {
                assert_eq!(statement, expected_kind, "for seed sql {sql:?}");
            },
            other => panic!("expected InvalidSeedStatement for {sql:?}, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn install_rejects_seed_with_unclassified_statement_as_other() {
    if provider_and_db().await.is_none() {
        return;
    }
    let err = seed_rejection("SET search_path TO public;").await;
    assert!(
        matches!(err, LoaderError::InvalidSeedStatement { statement, .. } if statement == "OTHER")
    );
}

#[tokio::test]
async fn install_rejects_unparseable_seed_sql() {
    if provider_and_db().await.is_none() {
        return;
    }
    let err = seed_rejection("THIS IS NOT SQL AT ALL").await;
    match err {
        LoaderError::SeedFailed { message, .. } => {
            assert!(message.contains("parse"), "message: {message}");
        },
        other => panic!("expected SeedFailed(parse), got {other:?}"),
    }
}

#[tokio::test]
async fn install_surfaces_seed_execution_failure_and_rolls_back() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_seed_exec_fail");
    let missing = unique_id("no_such_table");
    let ext = StubExtension {
        id: unique_id("ext_seed_exec_fail"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![Seed::new(
            unique_id("seed"),
            leak(format!(
                "INSERT INTO \"{table}\" (id) VALUES (7) ON CONFLICT (id) DO NOTHING; INSERT \
                 INTO \"{missing}\" (id) VALUES (1) ON CONFLICT (id) DO NOTHING;"
            )),
        )],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("seed hitting a missing table must fail");
    match err {
        LoaderError::SeedFailed { message, .. } => {
            assert!(message.contains("execute"), "message: {message}");
        },
        other => panic!("expected SeedFailed(execute), got {other:?}"),
    }

    let pg = db.write_pool_arc().expect("write pool");
    let rows: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM \"{table}\""
    )))
    .fetch_one(&*pg)
    .await
    .expect("count");
    assert_eq!(rows, 0, "failed seed transaction must roll back");

    drop_table(&db, table).await;
}

#[tokio::test]
async fn install_applies_update_and_multi_statement_seed() {
    let Some((provider, db)) = provider_and_db().await else {
        return;
    };
    let table = unique_id("install_seed_update");
    let ext = StubExtension {
        id: unique_id("ext_seed_update"),
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY, label TEXT);"),
        )],
        seeds: vec![Seed::new(
            unique_id("seed"),
            leak(format!(
                "INSERT INTO \"{table}\" (id, label) VALUES (1, 'raw') ON CONFLICT (id) DO \
                 NOTHING; UPDATE \"{table}\" SET label = 'updated' WHERE id = 1;"
            )),
        )],
    };

    install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect("multi-statement seed applies");

    let pg = db.write_pool_arc().expect("write pool");
    let label: String = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT label FROM \"{table}\" WHERE id = 1"
    )))
    .fetch_one(&*pg)
    .await
    .expect("label");
    assert_eq!(label, "updated");

    drop_table(&db, table).await;
}
