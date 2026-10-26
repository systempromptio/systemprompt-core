//! The owned boot sequence: profile, secrets, config, services root.
//!
//! [`BootContext`] is what the entry crates build once at startup and hand to
//! every reader — the CLI command context, the API server and the
//! `AppContext` builder. It holds the loaded profile and the secrets service,
//! and the stages that follow them (the resolved [`Config`] and the services
//! root) as write-once cells filled by the `try_init_*`
//! methods in boot order. A stage that has not run answers
//! `NotInitialized`; a stage that has run keeps its first value, so the
//! methods are safe to call from every path that may be the first to need it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use systemprompt_config::{ConfigError, ConfigResult, LoadedProfile, SecretsService};
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::Profile;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_models::errors::GlobalConfigError;

use crate::bundle::{BundleResult, ServicesSourceBootstrap};
use crate::config_loader::ConfigLoader;
use crate::error::ConfigLoadResult;
use systemprompt_manifest::services::root::ActiveServicesRoot;

#[derive(Clone)]
pub struct BootContext {
    inner: Arc<BootInner>,
}

struct BootInner {
    profile: Arc<LoadedProfile>,
    secrets: Arc<SecretsService>,
    config: OnceLock<Arc<Config>>,
    services_root: OnceLock<ActiveServicesRoot>,
}

impl std::fmt::Debug for BootContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootContext")
            .field("profile", &self.inner.profile.path())
            .field("config", &self.inner.config.get().is_some())
            .field("services_root", &self.inner.services_root.get())
            .finish_non_exhaustive()
    }
}

impl BootContext {
    #[must_use]
    pub fn new(profile: Arc<LoadedProfile>, secrets: Arc<SecretsService>) -> Self {
        Self {
            inner: Arc::new(BootInner {
                profile,
                secrets,
                config: OnceLock::new(),
                services_root: OnceLock::new(),
            }),
        }
    }

    #[must_use]
    pub fn profile(&self) -> &Profile {
        self.inner.profile.profile()
    }

    #[must_use]
    pub fn loaded_profile(&self) -> &Arc<LoadedProfile> {
        &self.inner.profile
    }

    #[must_use]
    pub fn profile_path(&self) -> &Path {
        self.inner.profile.path()
    }

    #[must_use]
    pub fn secrets(&self) -> &Arc<SecretsService> {
        &self.inner.secrets
    }

    pub fn config(&self) -> Result<&Arc<Config>, GlobalConfigError> {
        self.inner
            .config
            .get()
            .ok_or(GlobalConfigError::NotInitialized)
    }

    #[must_use]
    pub fn is_config_initialized(&self) -> bool {
        self.inner.config.get().is_some()
    }

    pub fn try_init_config(&self, services_root: Option<&Path>) -> ConfigResult<&Arc<Config>> {
        if let Some(config) = self.inner.config.get() {
            return Ok(config);
        }
        let config = systemprompt_config::build_from_profile(
            &self.inner.profile,
            &self.inner.secrets.get(),
            services_root,
        )?;
        self.install_config(config)
    }

    pub fn install_config(&self, config: Config) -> ConfigResult<&Arc<Config>> {
        self.inner
            .config
            .set(Arc::new(config))
            .map_err(|_config| ConfigError::AlreadyInitialized)?;
        self.inner
            .config
            .get()
            .ok_or_else(|| ConfigError::AlreadyInitialized)
    }

    #[must_use]
    pub fn services_root(&self) -> Option<&ActiveServicesRoot> {
        self.inner.services_root.get()
    }

    pub fn require_services_root(&self) -> ActiveServicesRoot {
        self.inner
            .services_root
            .get()
            .cloned()
            .unwrap_or_else(|| ActiveServicesRoot::baked(self.profile()))
    }

    pub fn install_services_root(&self, root: ActiveServicesRoot) -> &ActiveServicesRoot {
        self.inner.services_root.get_or_init(|| root)
    }

    pub async fn try_resolve_services_root(
        &self,
        core_version: &str,
    ) -> BundleResult<&ActiveServicesRoot> {
        if let Some(active) = self.inner.services_root.get() {
            return Ok(active);
        }
        let secrets = self.inner.secrets.get();
        let root = ServicesSourceBootstrap::resolve(
            self.profile(),
            |name| secrets.get(name).cloned(),
            core_version,
        )
        .await?;
        Ok(self.install_services_root(root))
    }

    pub fn load_services_config(&self) -> ConfigLoadResult<ServicesConfig> {
        self.config_loader().load()
    }

    #[must_use]
    pub fn config_loader(&self) -> ConfigLoader {
        ConfigLoader::for_profile(self.profile(), &self.require_services_root().path)
    }
}
