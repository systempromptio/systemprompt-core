//! A throwaway Postgres database owned by one test.
//!
//! Any test that *installs* a schema needs a database it created for the run.
//! Installing into the shared measurement database makes the run's outcome
//! depend on what earlier runs left there: an install interrupted part-way
//! leaves tables behind, the next install reads them and takes the
//! established path, and the test fails on a database nothing is visibly
//! wrong with. A database created for the test cannot carry that history.
//!
//! [`DisposableDb::empty`] hands back an empty database; [`DisposableDb::
//! with_schema`] hands back one with every registered extension's schema
//! applied; [`DisposableDb::test_pool`] connects to it. All three panic on
//! failure: a test that cannot get its database fails rather than skips (see
//! [`crate::db`]). Each database is dropped by [`DisposableDb::drop_now`],
//! and any a test never dropped is removed by a later run once its owner has
//! exited ([`crate::orphans`]).

use anyhow::{Context, Result};
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;

use crate::db::{connect, test_database_url};

pub struct DisposableDb {
    admin: sqlx::PgPool,
    name: String,
    url: String,
}

impl DisposableDb {
    // Why: the name carries the caller's prefix so a database says which
    // suite made it, its owner's PID so a later run can drop it once that
    // process is gone (a panicking test never reaches `drop_now`), and a
    // random suffix so parallel tests never collide.
    async fn create(prefix: &str) -> Result<Self> {
        let base_url = test_database_url();
        let admin = connect(&base_url).await?.pool().as_ref().clone();

        crate::orphans::sweep_databases(&admin).await;
        let name = crate::orphans::database_name(prefix);
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
    async fn installed(prefix: &str) -> Result<Self> {
        let db = Self::create(prefix).await?;
        let pool = connect(&db.url).await?;
        systemprompt_database::install_extension_schemas_full(
            &ExtensionRegistry::discover().context("extension registry discovery")?,
            pool.write(),
            &[],
            systemprompt_database::MigrationConfig::default(),
        )
        .await
        .context("failed to install the extension schemas into the disposable database")?;
        Ok(db)
    }

    pub async fn empty(prefix: &str) -> Self {
        Self::create(prefix)
            .await
            .unwrap_or_else(|e| panic!("disposable database `{prefix}`: {e:#}"))
    }

    pub async fn with_schema(prefix: &str) -> Self {
        Self::installed(prefix)
            .await
            .unwrap_or_else(|e| panic!("installed disposable database `{prefix}`: {e:#}"))
    }

    // Why: a pool per caller rather than one held on the struct -- a sqlx
    // connection belongs to the runtime that opened it, so a pool shared
    // across `#[tokio::test]` runtimes hands out dead sockets.
    pub async fn test_pool(&self) -> DbPool {
        connect(&self.url)
            .await
            .unwrap_or_else(|e| panic!("disposable database `{}`: {e:#}", self.name))
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
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
