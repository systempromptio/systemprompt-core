//! Per-invocation command context.
//!
//! [`CommandContext`] is the single dependency handed to command handlers: the
//! resolved [`CliConfig`], the [`EnvOverrides`] snapshot, the active
//! [`Prompter`], and lazy access to the runtime. The runtime is resolved on
//! first use — `--database-url` invocations carry a [`DatabaseContext`] and
//! refuse to boot the full [`AppContext`]; profile-driven invocations boot the
//! [`AppContext`] once and share it across the command. Tests construct the
//! context with [`CommandContext::with_app_context`] or
//! [`CommandContext::with_database`] and a scripted prompter, so no handler
//! needs process-global state.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use systemprompt_database::DbPool;
use systemprompt_runtime::{AppContext, DatabaseContext};
use tokio::sync::OnceCell;

use crate::cli_settings::CliConfig;
use crate::env_overrides::EnvOverrides;
use crate::interactive::{DialoguerPrompter, Prompter};

pub struct CommandContext {
    pub cli: CliConfig,
    pub env: EnvOverrides,
    prompter: Box<dyn Prompter>,
    runtime: OnceCell<Arc<AppContext>>,
    db: Option<DatabaseContext>,
    database_url: Option<String>,
}

impl CommandContext {
    #[must_use]
    pub fn new(cli: CliConfig, env: EnvOverrides) -> Self {
        Self {
            cli,
            env,
            prompter: Box::new(DialoguerPrompter),
            runtime: OnceCell::new(),
            db: None,
            database_url: None,
        }
    }

    #[must_use]
    pub fn with_database(
        cli: CliConfig,
        env: EnvOverrides,
        db: DatabaseContext,
        database_url: String,
    ) -> Self {
        Self {
            cli,
            env,
            prompter: Box::new(DialoguerPrompter),
            runtime: OnceCell::new(),
            db: Some(db),
            database_url: Some(database_url),
        }
    }

    #[must_use]
    pub fn with_app_context(cli: CliConfig, env: EnvOverrides, app: Arc<AppContext>) -> Self {
        let runtime = OnceCell::new();
        // Why: `OnceCell::set` cannot fail on a freshly constructed cell; the
        // discard keeps the constructor infallible.
        let _ = runtime.set(app);
        Self {
            cli,
            env,
            prompter: Box::new(DialoguerPrompter),
            runtime,
            db: None,
            database_url: None,
        }
    }

    #[must_use]
    pub fn with_prompter(mut self, prompter: Box<dyn Prompter>) -> Self {
        self.prompter = prompter;
        self
    }

    #[must_use]
    pub fn prompter(&self) -> &dyn Prompter {
        self.prompter.as_ref()
    }

    #[must_use]
    pub const fn is_database_scoped(&self) -> bool {
        self.db.is_some()
    }

    #[must_use]
    pub fn database_url(&self) -> Option<&str> {
        self.database_url.as_deref()
    }

    #[must_use]
    pub const fn database_context(&self) -> Option<&DatabaseContext> {
        self.db.as_ref()
    }

    pub async fn app_context(&self) -> Result<&Arc<AppContext>> {
        if self.db.is_some() {
            bail!(
                "This command requires full profile initialization. Remove --database-url flag."
            );
        }
        self.runtime
            .get_or_try_init(|| async { AppContext::new().await.map(Arc::new) })
            .await
            .context("Failed to initialize application context")
    }

    pub async fn db_pool(&self) -> Result<DbPool> {
        if let Some(db) = &self.db {
            return Ok(db.db_pool_arc());
        }
        Ok(self.app_context().await?.db_pool().clone())
    }

    pub async fn database(&self) -> Result<DatabaseContext> {
        if let Some(db) = &self.db {
            return Ok(db.clone());
        }
        Ok(DatabaseContext::from_pool(
            self.app_context().await?.db_pool().clone(),
        ))
    }
}
