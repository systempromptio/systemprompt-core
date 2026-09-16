//! Fingerprint-keyed memo of the loaded disk catalogue and the assembled
//! plugin bundles, owned by whoever serves manifests (the application context)
//! rather than by the process. A miss recomputes and replaces the single
//! retained entry; a fingerprint covers the services config, the external
//! URL and the metadata of every directory the entry was built from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use systemprompt_models::bridge::ids::PluginId;
use systemprompt_models::services::ServicesConfig;

use super::content::{CatalogContent, catalog_fingerprint};
use super::plugins::{bundle_fingerprint, plugin_bundles};
use crate::bundle::{BundleContent, PluginBundle};
use crate::error::MarketplaceError;

pub type BundleMap = BTreeMap<PluginId, PluginBundle>;

type Slot<T> = RwLock<Option<([u8; 32], Arc<T>)>>;

#[derive(Default)]
pub struct MarketplaceCache {
    catalog: Slot<CatalogContent>,
    bundles: Slot<BundleMap>,
}

impl std::fmt::Debug for MarketplaceCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketplaceCache").finish_non_exhaustive()
    }
}

impl MarketplaceCache {
    pub fn catalog(
        &self,
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
    ) -> Result<Arc<CatalogContent>, MarketplaceError> {
        let fingerprint = catalog_fingerprint(services, services_root, api_external_url)?;
        memo(&self.catalog, fingerprint, || {
            CatalogContent::load(services, services_root, api_external_url)
        })
    }

    pub fn bundles(
        &self,
        services: &ServicesConfig,
        content: &BundleContent<'_>,
    ) -> Result<Arc<BundleMap>, MarketplaceError> {
        let fingerprint = bundle_fingerprint(services, content)?;
        memo(&self.bundles, fingerprint, || {
            plugin_bundles(services, content)
        })
    }
}

fn memo<T>(
    slot: &Slot<T>,
    fingerprint: [u8; 32],
    load: impl FnOnce() -> Result<T, MarketplaceError>,
) -> Result<Arc<T>, MarketplaceError> {
    let hit = {
        let guard = slot
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .as_ref()
            .filter(|(cached, _)| *cached == fingerprint)
            .map(|(_, value)| Arc::clone(value))
    };
    if let Some(value) = hit {
        return Ok(value);
    }
    let value = Arc::new(load()?);
    let mut guard = slot
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some((fingerprint, Arc::clone(&value)));
    drop(guard);
    Ok(value)
}
