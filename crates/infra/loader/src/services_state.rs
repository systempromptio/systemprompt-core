//! The owned services-config state.
//!
//! [`ServicesState`] is built once by the boot path and handed to every
//! consumer: the merged, validated [`ServicesConfig`] — agents, MCP servers,
//! resolved gateway — behind a [`ConfigWatch`] the live-reload driver replaces,
//! beside the one [`ResolvedCatalog`] every consumer reads models, deployments
//! and credentials from. A load failure is a boot failure: nothing downstream
//! may run against a catalog that did not parse or resolve. The catalog's
//! `{install}` placeholders expand to the active profile's `site.name`
//! ([`catalog_context`]).
//!
//! Model discovery ([`ServicesState::load_with_discovery`]) is **boot-time
//! only**: the augmentation runs in the one window where the config is still
//! owned — between `ConfigLoader::load` and resolution — and the config is
//! re-validated afterwards so a discovered model can never bypass the gateway
//! pricing gate. A model that appears upstream mid process is served only after
//! the next restart, as is a catalog change. The report from that pass is kept
//! behind [`ServicesState::discovery_report`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use systemprompt_manifest::services::catalog::{CatalogContext, CatalogDocument, ResolvedCatalog};
use systemprompt_manifest::services::{DiscoveryReport, ServicesConfig, ServicesView};
use systemprompt_traits::ConfigWatch;

use systemprompt_manifest::profile::Profile;

use crate::boot::BootContext;
use crate::config_loader::ConfigLoader;
use crate::error::ConfigLoadResult;

pub type DiscoveryFuture<'a> = Pin<Box<dyn Future<Output = DiscoveryReport> + Send + 'a>>;

#[derive(Debug, Clone)]
pub struct ServicesState {
    watch: ConfigWatch<ServicesConfig>,
    catalog: Arc<ResolvedCatalog>,
    discovery: Option<Arc<DiscoveryReport>>,
}

impl ServicesState {
    pub fn load(boot: &BootContext) -> ConfigLoadResult<Self> {
        Self::from_config(boot.profile(), boot.load_services_config()?)
    }

    pub fn load_from_path(path: &Path, profile: &Profile) -> ConfigLoadResult<Self> {
        Self::from_config(profile, ConfigLoader::load_from_path(path, Some(profile))?)
    }

    // Why: the augmenter borrows the catalog across an await, so it is a
    // boxed future tied to that borrow — a plain `FnOnce(&mut _) -> Fut` cannot
    // name the lifetime and every real async fn fails the higher-ranked bound.
    pub async fn load_with_discovery<F>(boot: &BootContext, augment: F) -> ConfigLoadResult<Self>
    where
        F: for<'a> FnOnce(&'a mut CatalogDocument) -> DiscoveryFuture<'a>,
    {
        let mut services = boot.load_services_config()?;
        let mut document = take_catalog(&mut services);
        let report = augment(&mut document).await;
        restore_catalog(&mut services, document);
        services.validate()?;
        let mut state = Self::from_config(boot.profile(), services)?;
        state.discovery = Some(Arc::new(report));
        Ok(state)
    }

    pub fn from_config(profile: &Profile, services: ServicesConfig) -> ConfigLoadResult<Self> {
        let catalog = services.resolve_catalog(&catalog_context(profile))?;
        Ok(Self {
            watch: ConfigWatch::new(services),
            catalog: Arc::new(catalog),
            discovery: None,
        })
    }

    #[must_use]
    pub fn config(&self) -> Arc<ServicesConfig> {
        self.watch.get()
    }

    #[must_use]
    pub const fn watch(&self) -> &ConfigWatch<ServicesConfig> {
        &self.watch
    }

    #[must_use]
    pub fn view(&self) -> ServicesView {
        ServicesView {
            config: self.watch.clone(),
            catalog: Arc::clone(&self.catalog),
        }
    }

    #[must_use]
    pub const fn catalog(&self) -> &Arc<ResolvedCatalog> {
        &self.catalog
    }

    #[must_use]
    pub fn discovery_report(&self) -> Option<&DiscoveryReport> {
        self.discovery.as_deref()
    }
}

#[must_use]
pub fn catalog_context(profile: &Profile) -> CatalogContext {
    CatalogContext::for_install(profile.site.name.clone())
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
