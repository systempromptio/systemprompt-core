//! Integration test for [`MigrationService::squash_through`].
//!
//! Apply migrations 1, 2, 3 to a fresh extension namespace, squash through 2,
//! and assert the bookkeeping table reflects: row at version 0 with the
//! baseline checksum, no rows at versions 1 or 2, version 3 untouched.
//! Re-running the install with a registry whose squashed migrations have been
//! deleted (i.e. only versions >= 3 remain) must be a no-op.

use std::env;
use std::sync::Arc;

use sqlx::{PgPool, Row};
use systemprompt_database::{Database, MigrationService, install_extension_schemas};
use systemprompt_extension::{
    Extension, ExtensionMetadata, ExtensionRegistry, Migration, SchemaDefinition,
};
use uuid::Uuid;

const DEFAULT_DATABASE_URL: &str = "postgres://systemprompt_admin:\
                                    3e00fcdac26b5b731829e8737515db8f@localhost:5432/\
                                    systemprompt-web";

fn database_url() -> String {
    env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn fresh_suffix() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_string()
}

struct SquashFixture {
    pool: PgPool,
    table: &'static str,
    ext_id: &'static str,
}

impl Drop for SquashFixture {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let table = self.table;
        let ext_id = self.ext_id;
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                    "DROP TABLE IF EXISTS {table} CASCADE"
                )))
                .execute(&pool)
                .await;
                let _ = sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
                    .bind(ext_id)
                    .execute(&pool)
                    .await;
            });
        });
    }
}

struct SquashExtension {
    id: &'static str,
    schema_sql: &'static str,
    migrations: Vec<Migration>,
}

impl Extension for SquashExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "squash-test",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            self.id.to_string(),
            self.schema_sql.to_string(),
        )]
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn squash_through_retires_source_rows_and_writes_baseline_row() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak(format!("squash_test_{suffix}"));
    let ext_id: &'static str = leak(format!("squash-ext-{suffix}"));

    let _cleanup = SquashFixture {
        pool: pool.clone(),
        table,
        ext_id,
    };

    let schema_sql: &'static str = leak(format!(
        "CREATE TABLE IF NOT EXISTS {table} (\n    id TEXT PRIMARY KEY,\n    col_a TEXT,\n    \
         col_b TEXT,\n    col_c TEXT\n        );"
    ));

    let m1_sql: &'static str = leak(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY); ALTER TABLE {table} ADD COLUMN \
         IF NOT EXISTS col_a TEXT;"
    ));
    let m2_sql: &'static str = leak(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS col_b TEXT;"
    ));
    let m3_sql: &'static str = leak(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS col_c TEXT;"
    ));

    let ext_full = SquashExtension {
        id: ext_id,
        schema_sql,
        migrations: vec![
            Migration::new(1, "init_table", m1_sql),
            Migration::new(2, "add_col_b", m2_sql),
            Migration::new(3, "add_col_c", m3_sql),
        ],
    };

    let db_arc = Arc::new(db);
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(ext_full))
        .expect("registry accepts extension");

    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("install must apply migrations 1..=3");

    let applied_before: Vec<i32> = sqlx::query(
        "SELECT version FROM extension_migrations WHERE extension_id = $1 ORDER BY version",
    )
    .bind(ext_id)
    .fetch_all(&pool)
    .await
    .expect("read applied")
    .into_iter()
    .map(|r| r.get::<i32, _>("version"))
    .collect();
    assert_eq!(
        applied_before,
        vec![1, 2, 3],
        "all three migrations must be applied"
    );

    let ext_for_squash = SquashExtension {
        id: ext_id,
        schema_sql,
        migrations: vec![
            Migration::new(1, "init_table", m1_sql),
            Migration::new(2, "add_col_b", m2_sql),
            Migration::new(3, "add_col_c", m3_sql),
        ],
    };

    let migration_service = MigrationService::new(db_arc.write());
    let plan = migration_service
        .squash_through(&ext_for_squash, 2, true)
        .await
        .expect("squash_through must succeed");
    assert!(plan.applied, "applied=true must be reflected on the plan");
    assert_eq!(plan.through, 2);
    assert_eq!(plan.baseline_name, "baseline_v2");
    assert_eq!(plan.source_versions, vec![1, 2]);
    assert!(plan.baseline_sql.contains("col_a"));
    assert!(plan.baseline_sql.contains("col_b"));
    assert!(!plan.baseline_sql.contains("col_c"));

    let applied_after: Vec<(i32, String, String)> = sqlx::query(
        "SELECT version, name, checksum FROM extension_migrations WHERE extension_id = $1 ORDER \
         BY version",
    )
    .bind(ext_id)
    .fetch_all(&pool)
    .await
    .expect("read applied after squash")
    .into_iter()
    .map(|r| {
        (
            r.get::<i32, _>("version"),
            r.get::<String, _>("name"),
            r.get::<String, _>("checksum"),
        )
    })
    .collect();

    assert_eq!(
        applied_after.len(),
        2,
        "only baseline + version 3 should remain"
    );
    assert_eq!(applied_after[0].0, 0, "baseline must be at version 0");
    assert_eq!(applied_after[0].1, "baseline_v2");
    assert_eq!(applied_after[0].2, plan.baseline_checksum);
    assert_eq!(applied_after[1].0, 3, "version 3 row must be untouched");

    let baseline_sql_static: &'static str = leak(plan.baseline_sql.clone());
    let ext_post_squash = SquashExtension {
        id: ext_id,
        schema_sql,
        migrations: vec![
            Migration::new(0, "baseline_v2", baseline_sql_static),
            Migration::new(3, "add_col_c", m3_sql),
        ],
    };

    let mut registry_post = ExtensionRegistry::new();
    registry_post
        .register(Arc::new(ext_post_squash))
        .expect("registry accepts post-squash extension");

    install_extension_schemas(&registry_post, db_arc.as_ref())
        .await
        .expect("re-running install on a squashed DB must be a no-op");

    let applied_final: Vec<i32> = sqlx::query(
        "SELECT version FROM extension_migrations WHERE extension_id = $1 ORDER BY version",
    )
    .bind(ext_id)
    .fetch_all(&pool)
    .await
    .expect("read applied final")
    .into_iter()
    .map(|r| r.get::<i32, _>("version"))
    .collect();
    assert_eq!(applied_final, vec![0, 3], "no new rows after re-install");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn squash_refuses_when_target_range_not_fully_applied() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak(format!("squash_refuse_{suffix}"));
    let ext_id: &'static str = leak(format!("squash-refuse-{suffix}"));

    let _cleanup = SquashFixture {
        pool: pool.clone(),
        table,
        ext_id,
    };

    let schema_sql: &'static str = leak(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY, col_a TEXT, col_b TEXT);"
    ));
    let m1_sql: &'static str = leak(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY); ALTER TABLE {table} ADD COLUMN \
         IF NOT EXISTS col_a TEXT;"
    ));
    let m2_sql: &'static str = leak(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS col_b TEXT;"
    ));

    let ext_v1_only = SquashExtension {
        id: ext_id,
        schema_sql,
        migrations: vec![Migration::new(1, "m1", m1_sql)],
    };

    let db_arc = Arc::new(db);
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(ext_v1_only))
        .expect("registry accepts extension");
    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("install applies migration 1 only");

    let ext_for_squash = SquashExtension {
        id: ext_id,
        schema_sql,
        migrations: vec![
            Migration::new(1, "m1", m1_sql),
            Migration::new(2, "m2", m2_sql),
        ],
    };

    let migration_service = MigrationService::new(db_arc.write());
    let err = migration_service
        .squash_through(&ext_for_squash, 2, false)
        .await
        .expect_err("squash must refuse when migration 2 has not been applied");
    let msg = err.to_string();
    assert!(
        msg.contains("not applied"),
        "error message must mention the unapplied migration, got: {msg}"
    );
}
