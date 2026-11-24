//! A scratch Postgres schema owned by one test.
//!
//! A test that creates, alters or drops tables of its own must not do it in
//! the shared test database's `public` schema: a sibling test, a served
//! instance's scheduler or a sweep over `public` runs between its DDL and its
//! assertion, and a table it leaves behind changes what the next install sees.
//! [`ScratchSchema::create`] makes a uniquely named schema on the shared test
//! database; every pool it hands out ([`ScratchSchema::pool`],
//! [`ScratchSchema::db_pool`]) pins `search_path` to that schema alone, so
//! unqualified DDL and queries land there and nowhere else.
//! [`ScratchSchema::drop_now`] drops it, and one a panicking test never dropped
//! is removed by a later run once its owner has exited ([`crate::orphans`]).
//!
//! A test that installs the core schema composition, or that asserts on
//! objects in `public` by name, needs a whole database instead
//! ([`crate::DisposableDb`]).

use std::sync::Arc;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use systemprompt_database::{Database, DbPool, SchemaName};

use crate::db::{
    FIXTURE_POOL_ACQUIRE_TIMEOUT, FIXTURE_POOL_IDLE_TIMEOUT, FIXTURE_POOL_MAX_CONNECTIONS,
    budget_exhausted, test_database_url, test_pg_pool,
};

pub struct ScratchSchema {
    admin: PgPool,
    name: String,
}

impl ScratchSchema {
    pub async fn create(prefix: &str) -> Self {
        assert!(
            prefix
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            "a scratch schema prefix is lowercase ASCII, digits and `_`: {prefix:?}"
        );
        let admin = test_pg_pool().await;
        crate::orphans::sweep_schemas(&admin).await;
        let name = crate::orphans::database_name(prefix);
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap_or_else(|e| panic!("create the scratch schema {name}: {e}"));
        Self { admin, name }
    }

    pub async fn pool(&self, max_connections: u32) -> PgPool {
        let search_path = self.name.clone();
        PgPoolOptions::new()
            .max_connections(max_connections)
            .idle_timeout(FIXTURE_POOL_IDLE_TIMEOUT)
            .acquire_timeout(FIXTURE_POOL_ACQUIRE_TIMEOUT)
            .after_connect(move |connection, _| {
                let search_path = search_path.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path', $1, false)")
                        .bind(&search_path)
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("SELECT set_config('application_name', $1, false)")
                        .bind(&search_path)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&test_database_url())
            .await
            .unwrap_or_else(|e| {
                panic!(
                    "connect to the scratch schema {}: {e} ({})",
                    self.name,
                    budget_exhausted()
                )
            })
    }

    pub async fn db_pool(&self) -> DbPool {
        let pool = self.pool(FIXTURE_POOL_MAX_CONNECTIONS).await;
        Arc::new(Database::from_pools(Arc::new(pool), None))
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn schema_name(&self) -> SchemaName {
        SchemaName::try_new(self.name.clone()).unwrap_or_else(|e| {
            panic!(
                "the scratch schema name {} is an identifier: {e}",
                self.name
            )
        })
    }

    // Why: a drop that fails silently leaks a schema per run. The failure is
    // reported rather than asserted, so a test that has made its point is not
    // turned red by its own cleanup; the orphan sweep removes it later.
    pub async fn drop_now(self) {
        if let Err(e) = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP SCHEMA IF EXISTS \"{}\" CASCADE",
            self.name
        )))
        .execute(&self.admin)
        .await
        {
            eprintln!("LEAKED scratch schema {}: {e}", self.name);
        }
    }
}
