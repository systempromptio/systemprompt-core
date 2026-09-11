//! Process-wide services-config bootstrap.
//!
//! Mirrors `systemprompt_config::ProfileBootstrap` for the services tree: the
//! merged, validated [`ServicesConfig`] — provider catalog, resolved gateway,
//! agents, MCP servers — is loaded once, right after the profile, and read
//! through `&'static` accessors for the life of the process. A load failure is
//! a boot failure: nothing downstream may run against a catalog that did not
//! parse.
//!
//! Provider discovery ([`ServicesBootstrap::try_init_with_discovery`]) is
//! therefore **boot-time only**. The registry lives behind a [`OnceLock`] and
//! is handed out as `&'static ProviderRegistry` to ~15 call sites, so there is
//! no sound way to mutate it after install; a model that appears upstream mid
//! process is served only after the next restart. The augmentation runs in the
//! one window where the config is still owned — between `ConfigLoader::load`
//! and `install` — and the config is re-validated afterwards so a discovered
//! model can never bypass the gateway pricing gate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::OnceLock;

use systemprompt_models::services::{
    DiscoveryReport, GatewayConfig, ProviderRegistry, ServicesConfig,
};

use crate::config_loader::ConfigLoader;
use crate::error::{ConfigLoadError, ConfigLoadResult};

static SERVICES: OnceLock<ServicesConfig> = OnceLock::new();
pub type DiscoveryFuture<'a> = Pin<Box<dyn Future<Output = DiscoveryReport> + Send + 'a>>;

static DISCOVERY: OnceLock<DiscoveryReport> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct ServicesBootstrap;

impl ServicesBootstrap {
    pub fn init() -> ConfigLoadResult<&'static ServicesConfig> {
        if SERVICES.get().is_some() {
            return Err(ConfigLoadError::AlreadyInitialized);
        }
        let services = ConfigLoader::load()?;
        Self::install(services)
    }

    pub fn init_from_path(path: &Path) -> ConfigLoadResult<&'static ServicesConfig> {
        if SERVICES.get().is_some() {
            return Err(ConfigLoadError::AlreadyInitialized);
        }
        let services = ConfigLoader::load_from_path(path)?;
        Self::install(services)
    }

    /// Load, let `augment` add discovered models to the registry, re-validate,
    /// then install. A no-op returning the installed config if a previous
    /// `init` already ran — discovery only ever happens on the first install.
    // Why: the augmenter borrows the registry across an await, so it is a
    // boxed future tied to that borrow — a plain `FnOnce(&mut _) -> Fut` cannot
    // name the lifetime and every real async fn fails the higher-ranked bound.
    pub async fn try_init_with_discovery<F>(augment: F) -> ConfigLoadResult<&'static ServicesConfig>
    where
        F: for<'a> FnOnce(&'a mut ProviderRegistry) -> DiscoveryFuture<'a>,
    {
        if let Some(services) = SERVICES.get() {
            return Ok(services);
        }
        let mut services = ConfigLoader::load()?;
        let report = augment(&mut services.providers).await;
        // Why: the pricing gate in `GatewayConfig::validate` is the only thing
        // standing between a freshly discovered model and an uncosted request,
        // so it must run again over the augmented registry, not just over the
        // YAML that `ConfigLoader::load` already checked.
        services
            .validate()
            .map_err(|e| ConfigLoadError::Validation(e.to_string()))?;
        let installed = Self::install(services)?;
        let _ = DISCOVERY.set(report);
        Ok(installed)
    }

    /// The report produced by the boot-time discovery pass, if one ran.
    #[must_use]
    pub fn discovery_report() -> Option<&'static DiscoveryReport> {
        DISCOVERY.get()
    }

    pub fn try_init() -> ConfigLoadResult<&'static ServicesConfig> {
        if let Some(services) = SERVICES.get() {
            return Ok(services);
        }
        Self::init()
    }

    pub fn get() -> ConfigLoadResult<&'static ServicesConfig> {
        SERVICES.get().ok_or(ConfigLoadError::NotInitialized)
    }

    pub fn providers() -> ConfigLoadResult<&'static ProviderRegistry> {
        Self::get().map(|s| &s.providers)
    }

    pub fn gateway() -> ConfigLoadResult<Option<&'static GatewayConfig>> {
        Self::get().map(ServicesConfig::gateway_config)
    }

    #[must_use]
    pub fn is_initialized() -> bool {
        SERVICES.get().is_some()
    }

    fn install(services: ServicesConfig) -> ConfigLoadResult<&'static ServicesConfig> {
        SERVICES
            .set(services)
            .map_err(|_already| ConfigLoadError::AlreadyInitialized)?;
        SERVICES.get().ok_or(ConfigLoadError::NotInitialized)
    }
}
