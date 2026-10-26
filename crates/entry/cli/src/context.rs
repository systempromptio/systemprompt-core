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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use systemprompt_cloud::CloudCredentials;
use systemprompt_config::{LoadedProfile, SecretsService};
use systemprompt_database::DbPool;
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_manifest::{Config, Profile};
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
    database_url_override: Option<String>,
    cloud_credentials: Option<CloudCredentials>,
    services: Option<ServicesState>,
    profile: Option<Arc<LoadedProfile>>,
    boot: Option<BootContext>,
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
            database_url_override: None,
            cloud_credentials: None,
            services: None,
            profile: None,
            boot: None,
        }
    }

    #[must_use]
    pub fn with_cloud_credentials(mut self, credentials: Option<CloudCredentials>) -> Self {
        self.cloud_credentials = credentials;
        self
    }

    #[must_use]
    pub fn with_database_url_override(mut self, url: Option<String>) -> Self {
        self.database_url_override = url;
        self
    }

    #[must_use]
    pub fn with_services(mut self, services: Option<ServicesState>) -> Self {
        self.services = services;
        self
    }

    #[must_use]
    pub const fn services(&self) -> Option<&ServicesState> {
        self.services.as_ref()
    }

    #[must_use]
    pub fn with_profile(mut self, profile: Option<Arc<LoadedProfile>>) -> Self {
        self.profile = profile;
        self
    }

    #[must_use]
    pub fn with_boot(mut self, boot: Option<BootContext>) -> Self {
        if let Some(boot) = &boot {
            self.profile = Some(Arc::clone(boot.loaded_profile()));
        }
        self.boot = boot;
        self
    }

    pub fn loaded_profile(&self) -> Result<&Arc<LoadedProfile>> {
        self.profile
            .as_ref()
            .context("This command requires a profile; pass --profile or run 'systemprompt admin session switch <profile>'")
    }

    pub fn profile(&self) -> Result<&Profile> {
        self.loaded_profile().map(|loaded| loaded.profile())
    }

    pub fn boot(&self) -> Result<&BootContext> {
        self.boot
            .as_ref()
            .context("This command requires the profile secrets to be loaded")
    }

    pub fn secrets(&self) -> Result<&Arc<SecretsService>> {
        self.boot().map(BootContext::secrets)
    }

    pub fn config(&self) -> Result<&Arc<Config>> {
        Ok(self.boot()?.config()?)
    }

    #[must_use]
    pub const fn cloud_credentials(&self) -> Option<&CloudCredentials> {
        self.cloud_credentials.as_ref()
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
            database_url_override: None,
            cloud_credentials: None,
            services: None,
            profile: None,
            boot: None,
        }
    }

    #[must_use]
    pub fn with_app_context(cli: CliConfig, env: EnvOverrides, app: Arc<AppContext>) -> Self {
        Self {
            cli,
            env,
            prompter: Box::new(DialoguerPrompter),
            runtime: OnceCell::new_with(Some(app)),
            db: None,
            database_url: None,
            database_url_override: None,
            cloud_credentials: None,
            services: None,
            profile: None,
            boot: None,
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
            bail!("This command requires full profile initialization. Remove --database-url flag.");
        }
        let boot = self.boot()?.clone();
        self.runtime
            .get_or_try_init(|| async {
                let mut builder = self
                    .services
                    .clone()
                    .into_iter()
                    .fold(AppContext::builder(boot), |builder, services| {
                        builder.with_services(services)
                    });
                if let Some(url) = &self.database_url_override {
                    builder = builder.with_database_url(url.clone());
                }
                builder
                    .build()
                    .await
                    .map(Arc::new)
                    .context("Failed to initialize application context")
            })
            .await
    }

    pub async fn db_pool(&self) -> Result<DbPool> {
        if let Some(db) = &self.db {
            return Ok(db.db_pool_arc());
        }
        Ok(Arc::clone(self.app_context().await?.db_pool()))
    }

    pub async fn database(&self) -> Result<DatabaseContext> {
        if let Some(db) = &self.db {
            return Ok(db.clone());
        }
        Ok(DatabaseContext::from_pool(Arc::clone(
            self.app_context().await?.db_pool(),
        )))
    }
}

impl std::fmt::Debug for CommandContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandContext")
            .field("cli", &self.cli)
            .field("env", &self.env)
            .field("database_scoped", &self.db.is_some())
            .finish_non_exhaustive()
    }
}
