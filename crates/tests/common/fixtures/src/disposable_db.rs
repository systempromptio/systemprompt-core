//! A throwaway Postgres database owned by one test.
//!
//! Any test that *installs* a schema needs a database it created for the run.
//! Installing into the shared measurement database makes the run's outcome
//! depend on what earlier runs left there: an install interrupted part-way
//! leaves tables behind, the next install reads them and takes the
//! established path, and the test fails on a database nothing is visibly
//! wrong with. A database created for the test cannot carry that history.
//!
//! [`DisposableDb::create`] hands back an empty database; [`DisposableDb::
//! installed`] hands back one with every registered extension's schema
//! applied. Both are dropped by [`DisposableDb::drop_now`].

use anyhow::{Context, Result};
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;

use crate::db::{fixture_database_url, fixture_db_pool};

/// A database created for the calling test, addressed by its own URL.
pub struct DisposableDb {
    admin: sqlx::PgPool,
    name: String,
    url: String,
}

impl DisposableDb {
    // Why: the name carries the caller's prefix so a leaked database says
    // which suite leaked it, and a uuid so parallel tests never collide.
    pub async fn create(prefix: &str) -> Result<Self> {
        let base_url = fixture_database_url()?;
        let admin = fixture_db_pool(&base_url)
            .await?
            .pool_arc()
            .context("the maintenance pool must expose a raw handle")?
            .as_ref()
            .clone();

        let name = format!("{prefix}_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .with_context(|| format!("failed to create the disposable database {name}"))?;

        let (base, _old) = base_url
            .rsplit_once('/')
            .context("DATABASE_URL must name a database")?;
        let url = format!("{base}/{name}");
        Ok(Self { admin, name, url })
    }

    // Why: the schema is installed through the same entry point the server
    // boots with, so the database a test starts from is the shape a real
    // fresh install produces -- baseline stamps included.
    pub async fn installed(prefix: &str) -> Result<Self> {
        let db = Self::create(prefix).await?;
        let pool = db.pool().await?;
        systemprompt_database::install_extension_schemas(
            &ExtensionRegistry::discover().context("extension registry discovery")?,
            pool.write(),
        )
        .await
        .context("failed to install the extension schemas into the disposable database")?;
        Ok(db)
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    // Why: a pool per caller rather than one held on the struct -- a sqlx
    // connection belongs to the runtime that opened it, so a pool shared
    // across `#[tokio::test]` runtimes hands out dead sockets.
    pub async fn pool(&self) -> Result<DbPool> {
        fixture_db_pool(&self.url).await
    }

    // Why: a drop that fails silently leaks a database per run, and the leak
    // is invisible until the server is out of them. The failure is reported
    // rather than asserted: a test that has already made its point should not
    // be turned red by its own cleanup.
    pub async fn drop_now(self) {
        if let Err(e) = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await
        {
            eprintln!("LEAKED disposable database {}: {e}", self.name);
        }
    }
}
