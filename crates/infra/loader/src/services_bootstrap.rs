//! Process-wide services-config bootstrap.
//!
//! Mirrors `systemprompt_config::ProfileBootstrap` for the services tree: the
//! merged, validated [`ServicesConfig`] — model catalog, resolved gateway,
//! agents, MCP servers — is loaded once, right after the profile, and read
//! through `&'static` accessors for the life of the process, beside the one
//! [`ResolvedCatalog`] every consumer reads models, deployments and
//! credentials from ([`ServicesBootstrap::catalog`]). A load failure is a boot
//! failure: nothing downstream may run against a catalog that did not parse
//! or resolve. The catalog's `{install}` placeholders expand to the active
//! profile's `site.name` ([`catalog_context`]).
//!
//! Model discovery ([`ServicesBootstrap::try_init_with_discovery`]) is
//! therefore **boot-time only**. The catalog lives behind a [`OnceLock`] and
//! is handed out as `&'static` to many call sites, so there is no sound way
//! to mutate it after install; a model that appears upstream mid
//! process is served only after the next restart. The augmentation runs in the
//! one window where the config is still owned — between `ConfigLoader::load`
//! and `install` — and the config is re-validated afterwards so a discovered
//! model can never bypass the gateway pricing gate. A second
//! `try_init_with_discovery` after any init is a no-op returning the installed
//! config; the report from the one pass that did run is kept behind
//! [`ServicesBootstrap::discovery_report`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::OnceLock;

use systemprompt_manifest::services::catalog::{CatalogContext, CatalogDocument, ResolvedCatalog};
use systemprompt_manifest::services::{DiscoveryReport, GatewayConfig, ServicesConfig};

use crate::config_loader::ConfigLoader;
use crate::error::{ConfigLoadError, ConfigLoadResult};

struct Installed {
    services: ServicesConfig,
    catalog: ResolvedCatalog,
}

static SERVICES: OnceLock<Installed> = OnceLock::new();
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

    // Why: the augmenter borrows the catalog across an await, so it is a
    // boxed future tied to that borrow — a plain `FnOnce(&mut _) -> Fut` cannot
    // name the lifetime and every real async fn fails the higher-ranked bound.
    pub async fn try_init_with_discovery<F>(augment: F) -> ConfigLoadResult<&'static ServicesConfig>
    where
        F: for<'a> FnOnce(&'a mut CatalogDocument) -> DiscoveryFuture<'a>,
    {
        if let Some(installed) = SERVICES.get() {
            return Ok(&installed.services);
        }
        let mut services = ConfigLoader::load()?;
        let mut document = take_catalog(&mut services);
        let report = augment(&mut document).await;
        restore_catalog(&mut services, document);
        services.validate()?;
        let installed = Self::install(services)?;
        if DISCOVERY.set(report).is_err() {
            tracing::warn!(
                "catalog discovery report already recorded for this process; keeping the first"
            );
        }
        Ok(installed)
    }

    #[must_use]
    pub fn discovery_report() -> Option<&'static DiscoveryReport> {
        DISCOVERY.get()
    }

    pub fn try_init() -> ConfigLoadResult<&'static ServicesConfig> {
        if let Some(installed) = SERVICES.get() {
            return Ok(&installed.services);
        }
        Self::init()
    }

    pub fn get() -> ConfigLoadResult<&'static ServicesConfig> {
        SERVICES
            .get()
            .map(|installed| &installed.services)
            .ok_or(ConfigLoadError::NotInitialized)
    }

    pub fn catalog() -> ConfigLoadResult<&'static ResolvedCatalog> {
        SERVICES
            .get()
            .map(|installed| &installed.catalog)
            .ok_or(ConfigLoadError::NotInitialized)
    }

    pub fn gateway() -> ConfigLoadResult<Option<&'static GatewayConfig>> {
        Self::get().map(ServicesConfig::gateway_config)
    }

    #[must_use]
    pub fn is_initialized() -> bool {
        SERVICES.get().is_some()
    }

    fn install(services: ServicesConfig) -> ConfigLoadResult<&'static ServicesConfig> {
        let catalog = services.resolve_catalog(&catalog_context())?;
        SERVICES
            .set(Installed { services, catalog })
            .map_err(|_already| ConfigLoadError::AlreadyInitialized)?;
        Self::get()
    }
}

#[must_use]
pub fn catalog_context() -> CatalogContext {
    systemprompt_config::ProfileBootstrap::get().map_or_else(
        |_| CatalogContext::default(),
        |profile| CatalogContext::for_install(profile.site.name.clone()),
    )
}

fn take_catalog(services: &mut ServicesConfig) -> CatalogDocument {
    CatalogDocument {
        credentials: std::mem::take(&mut services.credentials),
        providers: std::mem::take(&mut services.providers),
        models: std::mem::take(&mut services.models),
        deployments: std::mem::take(&mut services.deployments),
        gateway: None,
    }
}

fn restore_catalog(services: &mut ServicesConfig, document: CatalogDocument) {
    services.credentials = document.credentials;
    services.providers = document.providers;
    services.models = document.models;
    services.deployments = document.deployments;
}
