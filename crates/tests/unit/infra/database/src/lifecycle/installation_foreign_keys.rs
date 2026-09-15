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
    // Why: the fixtures are several statements; a prepared statement takes one.
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
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

// Why: the 2026-09-14 incident — `parent` already exists without the composite
// unique index that migration 001 adds, so an inline key failed in the
// structural phase and only a key deferred past 001 can succeed.
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

#[tokio::test]
async fn an_established_extension_reports_an_uncreatable_key_as_typed_drift() {
    let Some((provider, db)) = provider_and_db_or_skip().await else {
        return;
    };
    let parent = unique_id("fk_drift_parent");
    let child = unique_id("fk_drift_child");
    let ext_id = unique_id("fk_drift_ext");

    // Why: an owned table already present with no migration history makes the
    // extension "established", so a key that cannot be created is drift.
    run_sql(
        &db,
        format!(
            "CREATE TABLE \"{parent}\" (id TEXT PRIMARY KEY, user_id TEXT NOT NULL);\n\
             CREATE TABLE \"{child}\" (id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, \
             parent_id TEXT NOT NULL);"
        ),
    )
    .await;
    forget_migrations(&db, ext_id).await;
    let schema_sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{child}\" (id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, \
         parent_id TEXT NOT NULL, FOREIGN KEY (owner_id, parent_id) REFERENCES \"{parent}\" \
         (user_id, id));"
    );
    let registry = registry_with(StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::sql_only(schema_sql)],
        seeds: vec![],
        migrations: vec![Migration::new(1, "noop", "SELECT 1")],
    });

    let report = install_extension_schemas_with_config(&registry, &provider, &[])
        .await
        .expect("drift on an established database does not fail the install");

    assert!(!report.is_clean());
    assert_eq!(report.foreign_key_drift.len(), 1);
    let drift = &report.foreign_key_drift[0];
    assert_eq!(drift.extension, ext_id);
    assert_eq!(drift.table, child);
    assert!(drift.sql.contains("FOREIGN KEY"), "{}", drift.sql);
    assert!(!drift.cause.is_empty());
    assert_eq!(foreign_key_count(&db, child).await, 0);

    forget_migrations(&db, ext_id).await;
    drop_table(&db, child).await;
    drop_table(&db, parent).await;
}

#[tokio::test]
async fn a_failing_catalog_probe_fails_the_install_on_an_established_database() {
    let Some((provider, db)) = provider_and_db_or_skip().await else {
        return;
    };
    let parent = unique_id("fk_probe_parent");
    let child = unique_id("fk_probe_child");
    let ext_id = unique_id("fk_probe_ext");

    run_sql(
        &db,
        format!(
            "CREATE TABLE \"{parent}\" (id TEXT PRIMARY KEY);\n\
             CREATE TABLE \"{child}\" (id TEXT PRIMARY KEY, parent_id TEXT NOT NULL);"
        ),
    )
    .await;
    forget_migrations(&db, ext_id).await;
    let schema_sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{child}\" (id TEXT PRIMARY KEY, parent_id TEXT NOT NULL \
         REFERENCES \"{parent}\" (id));"
    );
    let registry = registry_with(StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::sql_only(schema_sql)],
        seeds: vec![],
        migrations: vec![Migration::new(1, "noop", "SELECT 1")],
    });

    let failing = probe_failing::ProbeFailingProvider::new(provider);
    let err = install_extension_schemas_with_config(&registry, &failing, &[])
        .await
        .expect_err("a failing catalog probe is not drift");
    assert!(err.to_string().contains("could not be probed"), "{err}");
    assert_eq!(foreign_key_count(&db, child).await, 0);

    forget_migrations(&db, ext_id).await;
    drop_table(&db, child).await;
    drop_table(&db, parent).await;
}

mod probe_failing {
    use async_trait::async_trait;
    use systemprompt_database::{
        DatabaseInfo, DatabaseProvider, DatabaseResult, DatabaseTransaction, JsonRow,
        PostgresProvider, QueryResult, QuerySelector, RepositoryError, ToDbValue,
    };

    #[derive(Debug)]
    pub(super) struct ProbeFailingProvider {
        inner: PostgresProvider,
    }

    impl ProbeFailingProvider {
        pub(super) const fn new(inner: PostgresProvider) -> Self {
            Self { inner }
        }
    }

    struct ProbeFailingTx {
        inner: Box<dyn DatabaseTransaction>,
    }

    #[async_trait]
    impl DatabaseTransaction for ProbeFailingTx {
        async fn execute(
            &mut self,
            query: &dyn QuerySelector,
            params: &[&dyn ToDbValue],
        ) -> DatabaseResult<u64> {
            self.inner.execute(query, params).await
        }

        async fn fetch_all(
            &mut self,
            query: &dyn QuerySelector,
            params: &[&dyn ToDbValue],
        ) -> DatabaseResult<Vec<JsonRow>> {
            self.inner.fetch_all(query, params).await
        }

        async fn fetch_one(
            &mut self,
            query: &dyn QuerySelector,
            params: &[&dyn ToDbValue],
        ) -> DatabaseResult<JsonRow> {
            self.inner.fetch_one(query, params).await
        }

        async fn fetch_optional(
            &mut self,
            query: &dyn QuerySelector,
            params: &[&dyn ToDbValue],
        ) -> DatabaseResult<Option<JsonRow>> {
            if query.select_query().contains("pg_constraint") {
                return Err(RepositoryError::internal("catalog unavailable"));
            }
            self.inner.fetch_optional(query, params).await
        }

        async fn commit(self: Box<Self>) -> DatabaseResult<()> {
            self.inner.commit().await
        }

        async fn rollback(self: Box<Self>) -> DatabaseResult<()> {
            self.inner.rollback().await
        }
    }

    #[async_trait]
    impl DatabaseProvider for ProbeFailingProvider {
        fn get_postgres_pool(&self) -> std::sync::Arc<sqlx::PgPool> {
            self.inner.get_postgres_pool()
        }

        async fn execute(
            &self,
            query: &dyn QuerySelector,
            params: &[&dyn ToDbValue],
        ) -> DatabaseResult<u64> {
            self.inner.execute(query, params).await
        }

        async fn execute_raw(&self, sql: &str) -> DatabaseResult<()> {
            self.inner.execute_raw(sql).await
        }

        async fn fetch_all(
            &self,
            query: &dyn QuerySelector,
            params: &[&dyn ToDbValue],
        ) -> DatabaseResult<Vec<JsonRow>> {
            self.inner.fetch_all(query, params).await
        }

        async fn fetch_one(
            &self,
            query: &dyn QuerySelector,
            params: &[&dyn ToDbValue],
        ) -> DatabaseResult<JsonRow> {
            self.inner.fetch_one(query, params).await
        }

        async fn fetch_optional(
            &self,
            query: &dyn QuerySelector,
            params: &[&dyn ToDbValue],
        ) -> DatabaseResult<Option<JsonRow>> {
            self.inner.fetch_optional(query, params).await
        }

        async fn begin_transaction(&self) -> DatabaseResult<Box<dyn DatabaseTransaction>> {
            let inner = self.inner.begin_transaction().await?;
            Ok(Box::new(ProbeFailingTx { inner }))
        }

        async fn get_database_info(&self) -> DatabaseResult<DatabaseInfo> {
            self.inner.get_database_info().await
        }

        async fn test_connection(&self) -> DatabaseResult<()> {
            self.inner.test_connection().await
        }

        async fn execute_batch(&self, sql: &str) -> DatabaseResult<()> {
            self.inner.execute_batch(sql).await
        }

        async fn query_raw(&self, query: &dyn QuerySelector) -> DatabaseResult<QueryResult> {
            self.inner.query_raw(query).await
        }

        async fn query_raw_with(
            &self,
            query: &dyn QuerySelector,
            params: &[&dyn ToDbValue],
        ) -> DatabaseResult<QueryResult> {
            self.inner.query_raw_with(query, params).await
        }
    }
}
