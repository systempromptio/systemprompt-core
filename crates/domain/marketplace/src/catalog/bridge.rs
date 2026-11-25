//! The marketplace as the shared [`BridgeCatalogSource`]: one catalogue
//! resolution per user per sync.
//!
//! The disk catalogue with the managed overlay, the filtered candidate and
//! the plugin bundles, memoised in the [`MarketplaceCache`] and shared by the
//! manifest route and every plugin-file download that follows it.
//!
//! A `Fresh` request revalidates only a memo older than
//! [`FRESH_REVALIDATE_AFTER`]. A hit costs one query (the managed stamp) and
//! no catalogue work. Misses for one user are single-flight: a request that
//! waited on another's rebuild takes its result instead of starting its own,
//! so a memo that expires under load is rebuilt once, not once per waiting
//! request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::bridge_catalog::{
    BridgeCatalogError, BridgeCatalogSource, CatalogFreshness, CatalogScope,
    DynResolvedBridgeCatalog, IssuedConsumerCredential, ManifestEntries, ResolvedBridgeCatalog,
};
use systemprompt_identifiers::{DeviceCertId, ManagedResourceId, PluginId, UserId};
use systemprompt_models::bridge::manifest::PluginEntry;

use super::{MarketplaceCache, ResolvedCatalog, ResolvedKey};
use crate::contract::MarketplaceSeams;
use crate::error::MarketplaceError;
use crate::managed::ManagedRepository;
use crate::manifest::{AssembleRequest, ManifestService};
use crate::trace::NoopTrace;

// Why: How old a memo may be and still answer a `no-cache` request. A bridge
// probes on a 10–30 s tick with `no-cache`, and rebuilding on each one starved
// the pool; a connector the user just linked shows up at most this late.
pub const FRESH_REVALIDATE_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// The marketplace state a bridge catalogue resolution reads.
#[derive(Clone)]
pub struct MarketplaceBridgeCatalog {
    managed: Arc<ManagedRepository>,
    cache: Arc<MarketplaceCache>,
    seams: Arc<MarketplaceSeams>,
}

impl std::fmt::Debug for MarketplaceBridgeCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketplaceBridgeCatalog")
            .finish_non_exhaustive()
    }
}

impl MarketplaceBridgeCatalog {
    #[must_use]
    pub const fn new(
        managed: Arc<ManagedRepository>,
        cache: Arc<MarketplaceCache>,
        seams: Arc<MarketplaceSeams>,
    ) -> Self {
        Self {
            managed,
            cache,
            seams,
        }
    }

    async fn resolve_catalog(
        &self,
        scope: CatalogScope<'_>,
        user_id: &UserId,
        freshness: CatalogFreshness,
    ) -> Result<Arc<ResolvedCatalog>, MarketplaceError> {
        let cache = &self.cache;
        let services = scope.services;
        let services_root = scope.services_root;
        let (fingerprint, disk_catalog) =
            cache.catalog_with_fingerprint(services, services_root, scope.api_external_url)?;
        let owner = scope.owner;
        let managed = &self.managed;
        let managed_stamp = managed
            .managed_catalog_stamp(owner, user_id)
            .await
            .map_err(MarketplaceError::Managed)?;
        let key = ResolvedKey {
            catalog: fingerprint,
            user: user_id.clone(),
            managed_stamp,
        };
        let memo = match freshness {
            CatalogFreshness::Memo => cache.resolved(&key),
            CatalogFreshness::Fresh => std::time::Instant::now()
                .checked_sub(FRESH_REVALIDATE_AFTER)
                .and_then(|since| cache.resolved_built_since(&key, since)),
        };
        if let Some(hit) = memo {
            tracing::debug!(user_id = %user_id, "bridge: resolved catalogue served from memo");
            return Ok(hit);
        }

        let requested_at = std::time::Instant::now();
        let gate = cache.rebuild_gate(user_id);
        let _rebuilding = gate.lock().await;
        if let Some(shared) = cache.resolved_built_since(&key, requested_at) {
            tracing::debug!(user_id = %user_id, "bridge: resolved catalogue shared a concurrent rebuild");
            return Ok(shared);
        }

        let started = std::time::Instant::now();
        let filter = self.seams.filter();
        let catalog = (*disk_catalog)
            .clone()
            .with_organization_resources(managed.as_ref().clone(), owner, user_id)
            .await?;
        let candidate = ManifestService::assemble_candidate_from_catalog(
            catalog.clone(),
            &AssembleRequest {
                services,
                services_root,
                filter: filter.as_ref(),
                user_id,
                cache,
            },
            &mut NoopTrace,
        )
        .await?;
        let bundles = cache.bundles(services, &catalog.as_content())?;
        let resolved = Arc::new(ResolvedCatalog {
            catalog: Arc::new(catalog),
            candidate: Arc::new(candidate),
            bundles,
        });
        cache.store_resolved(key, Arc::clone(&resolved));
        tracing::info!(
            user_id = %user_id,
            elapsed_ms = started.elapsed().as_millis(),
            plugins = resolved.candidate.plugins.len(),
            skills = resolved.candidate.skills.len(),
            fresh = freshness == CatalogFreshness::Fresh,
            "bridge: resolved catalogue rebuilt"
        );
        Ok(resolved)
    }
}

#[async_trait]
impl BridgeCatalogSource for MarketplaceBridgeCatalog {
    async fn resolve(
        &self,
        scope: CatalogScope<'_>,
        user: &UserId,
        freshness: CatalogFreshness,
    ) -> Result<DynResolvedBridgeCatalog, BridgeCatalogError> {
        let resolved = self
            .resolve_catalog(scope, user, freshness)
            .await
            .map_err(|e| BridgeCatalogError::Resolve(Box::new(e)))?;
        Ok(resolved)
    }

    fn latest(&self, user: &UserId) -> Option<DynResolvedBridgeCatalog> {
        let latest: DynResolvedBridgeCatalog = self.cache.latest_resolved(user)?;
        Some(latest)
    }

    async fn retain_catalog_grants(
        &self,
        owner: &UserId,
        resources: &[ManagedResourceId],
        user: &UserId,
    ) -> Result<(), BridgeCatalogError> {
        self.managed
            .retain_consumer_catalog_grants(owner, resources, user)
            .await
            .map_err(|e| BridgeCatalogError::Grants(Box::new(e)))
    }

    async fn issue_consumer_credential(
        &self,
        cert: &DeviceCertId,
    ) -> Result<IssuedConsumerCredential, BridgeCatalogError> {
        self.managed
            .issue_consumer_credential(cert)
            .await
            .map_err(|e| BridgeCatalogError::Credential(Box::new(e)))
    }
}

impl ResolvedBridgeCatalog for ResolvedCatalog {
    fn plugins(&self) -> &[PluginEntry] {
        &self.candidate.plugins
    }

    fn skill_publications(&self) -> Vec<ManagedResourceId> {
        self.candidate
            .skills
            .iter()
            .filter_map(|skill| skill.publication.as_ref())
            .map(|publication| publication.resource_id.clone())
            .collect()
    }

    fn manifest_entries(&self) -> ManifestEntries {
        (*self.candidate).clone().into_manifest_parts().0
    }

    fn plugin_file(&self, plugin: &PluginId, path: &str) -> Option<Option<&[u8]>> {
        let bundle = self.bundles.get(plugin)?;
        Some(bundle.get(path).map(|file| file.bytes.as_slice()))
    }

    // Why: one entry per bundle path, mode `0755` for executables.
    fn pack_plugin(&self, plugin: &PluginId) -> Option<Result<Vec<u8>, BridgeCatalogError>> {
        let bundle = self.bundles.get(plugin)?;
        Some(
            systemprompt_loader::bundle::pack::create_tarball_from_memory(
                bundle
                    .iter()
                    .map(|(path, file)| (path.as_str(), file.bytes.as_slice(), file.executable)),
            )
            .map_err(|e| BridgeCatalogError::Pack(Box::new(e))),
        )
    }
}
