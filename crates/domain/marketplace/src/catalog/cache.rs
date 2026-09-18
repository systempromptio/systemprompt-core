//! Fingerprint-keyed memo of the loaded disk catalogue and the assembled
//! plugin bundles, owned by whoever serves manifests (the application context)
//! rather than by the process. A miss recomputes and replaces the single
//! retained entry; a fingerprint covers the services config, the external
//! URL and the metadata of every directory the entry was built from.
//!
//! The per-user slot memoises the *resolved* catalogue — disk catalogue with
//! the managed overlay applied, the assembled candidate and its bundles — so
//! the manifest route and every plugin-file download of one sync share one
//! resolution instead of each rebuilding it. An entry is keyed by the disk
//! fingerprint, the user and a stamp of the managed tables it read, and
//! expires after [`RESOLVED_TTL`] regardless, so a state the stamp does not
//! cover — the user's `MarketplaceFilter` (a DB-backed ACL) and a grant that
//! is new rather than revoked — can never be served for longer than that.
//!
//! The cache also remembers which catalogue-shape warnings it has logged, so
//! a resolution that runs once per user per minute reports each once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::ids::PluginId;
use systemprompt_models::services::ServicesConfig;

use super::content::{CatalogContent, catalog_fingerprint};
use super::plugins::{bundle_fingerprint, plugin_bundles};
use crate::bundle::{BundleContent, PluginBundle};
use crate::candidate::MarketplaceCandidate;
use crate::error::MarketplaceError;

pub type BundleMap = BTreeMap<PluginId, PluginBundle>;

type Slot<T> = RwLock<Option<([u8; 32], Arc<T>)>>;

pub const RESOLVED_TTL: Duration = Duration::from_secs(60);

pub const RESOLVED_CAPACITY: usize = 16;

/// What a resolved entry was built from. Two requests share an entry only
/// when every field matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedKey {
    pub catalog: [u8; 32],
    pub user: UserId,
    pub managed_stamp: String,
}

/// One user's fully resolved catalogue: the overlaid content, the filtered
/// candidate the manifest is built from, and the bundles its bytes come from.
#[derive(Debug)]
pub struct ResolvedCatalog {
    pub catalog: Arc<CatalogContent>,
    pub candidate: Arc<MarketplaceCandidate>,
    pub bundles: Arc<BundleMap>,
}

struct ResolvedEntry {
    key: ResolvedKey,
    built_at: Instant,
    value: Arc<ResolvedCatalog>,
}

#[derive(Default)]
pub struct MarketplaceCache {
    catalog: Slot<CatalogContent>,
    bundles: Slot<BundleMap>,
    resolved: RwLock<VecDeque<ResolvedEntry>>,
    sighted: Mutex<BTreeSet<String>>,
}

const SIGHTED_CAPACITY: usize = 4096;

impl std::fmt::Debug for MarketplaceCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketplaceCache").finish_non_exhaustive()
    }
}

impl MarketplaceCache {
    // Why: a resolution runs once per user per minute, and every one of them
    // would repeat the same catalogue-shape warnings; the first sighting of a
    // (kind, id) is the operator's signal, the rest is noise at debug.
    pub fn first_sighting(&self, kind: &str, id: &str) -> bool {
        let mut seen = self
            .sighted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if seen.len() >= SIGHTED_CAPACITY {
            seen.clear();
        }
        seen.insert(format!("{kind}:{id}"))
    }

    pub fn catalog(
        &self,
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
    ) -> Result<Arc<CatalogContent>, MarketplaceError> {
        self.catalog_with_fingerprint(services, services_root, api_external_url)
            .map(|(_, catalog)| catalog)
    }

    pub fn catalog_with_fingerprint(
        &self,
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
    ) -> Result<([u8; 32], Arc<CatalogContent>), MarketplaceError> {
        let fingerprint = catalog_fingerprint(services, services_root, api_external_url)?;
        let catalog = memo(&self.catalog, fingerprint, || {
            CatalogContent::load(services, services_root, api_external_url)
        })?;
        Ok((fingerprint, catalog))
    }

    pub fn resolved(&self, key: &ResolvedKey) -> Option<Arc<ResolvedCatalog>> {
        self.resolved_at(key, Instant::now())
    }

    pub fn resolved_at(&self, key: &ResolvedKey, now: Instant) -> Option<Arc<ResolvedCatalog>> {
        let guard = self
            .resolved
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .iter()
            .find(|entry| &entry.key == key)
            .filter(|entry| now.saturating_duration_since(entry.built_at) < RESOLVED_TTL)
            .map(|entry| Arc::clone(&entry.value))
    }

    pub fn store_resolved(&self, key: ResolvedKey, value: Arc<ResolvedCatalog>) {
        self.store_resolved_at(key, value, Instant::now());
    }

    // Why: one entry per user — a user's stale key is replaced, never kept
    // beside its successor — and the deque is the eviction order.
    pub fn store_resolved_at(
        &self,
        key: ResolvedKey,
        value: Arc<ResolvedCatalog>,
        built_at: Instant,
    ) {
        let mut guard = self
            .resolved
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.retain(|entry| entry.key.user != key.user);
        guard.push_back(ResolvedEntry {
            key,
            built_at,
            value,
        });
        while guard.len() > RESOLVED_CAPACITY {
            guard.pop_front();
        }
        drop(guard);
    }

    pub fn resolved_len(&self) -> usize {
        self.resolved
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
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
