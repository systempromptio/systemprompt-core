//! [`InventoryRefresher`]: the configured-owner inventory refresh, bound to
//! the services tree and the profile's bundle sources.
//!
//! The `managed_resources` member builds one refresher in its provide fn
//! from the provided repository and publish guard, the sealed system admin,
//! active services root and [`BundleSources`] kernel seeds, and the open
//! services view. The refresher is provided only as the member's own
//! [`MemberRefresher`], a type no other crate can name, so only the
//! member's `managed_inventory_refresh` job resolves it. A kernel route
//! reaches the refresher only as the sealed `DynInventoryPublisher`
//! (`publish_latest` with the authenticated request's user as the actor);
//! no member runtime context resolves it. Neither reads the
//! profile, the configuration or a secret: everything a refresh reads from
//! the profile is the [`BundleSources`] value inside it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_loader::bundle::BundleSources;
use systemprompt_manifest::services::ServicesView;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_traits::{InventoryPublishError, InventoryPublisher};
use tokio::sync::Mutex;

use super::{
    BaselineScope, InventoryService, InventoryStatus, LatestPublication, LatestPublicationStatus,
    PublishGuard, SourcesContext, configured_inventory_fingerprint,
};
use crate::managed::{ManagedRepository, Result};

/// The handles an [`InventoryRefresher`] is built from.
#[derive(Debug)]
pub struct InventoryRefresherParts {
    pub repository: Arc<ManagedRepository>,
    pub guard: Arc<Mutex<PublishGuard>>,
    pub bundles: BundleSources,
    pub active_root: ActiveServicesRoot,
    pub services: ServicesView,
    pub owner: UserId,
}

/// Reconciles and publishes the configured owner's inventory.
#[derive(Debug)]
pub struct InventoryRefresher {
    service: InventoryService,
    guard: Arc<Mutex<PublishGuard>>,
    bundles: BundleSources,
    active_root: ActiveServicesRoot,
    services: ServicesView,
    owner: UserId,
}

impl InventoryRefresher {
    #[must_use]
    pub fn new(parts: InventoryRefresherParts) -> Self {
        Self {
            service: InventoryService::new(parts.repository.as_ref().clone()),
            guard: parts.guard,
            bundles: parts.bundles,
            active_root: parts.active_root,
            services: parts.services,
            owner: parts.owner,
        }
    }

    #[must_use]
    pub const fn owner(&self) -> &UserId {
        &self.owner
    }

    fn root(&self) -> PathBuf {
        self.active_root.path.clone()
    }

    const fn sources(&self) -> SourcesContext<'_> {
        SourcesContext {
            bundles: &self.bundles,
            active_root: Some(&self.active_root),
        }
    }

    pub fn fingerprint(&self) -> Result<[u8; 32]> {
        configured_inventory_fingerprint(&self.root(), &self.services.config())
    }

    pub async fn refresh(&self) -> Result<InventoryStatus> {
        let services = self.services.config();
        self.service
            .refresh(&self.owner, &self.root(), &services, self.sources())
            .await
    }

    pub async fn publish_latest(&self, actor: &UserId) -> Result<Vec<LatestPublication>> {
        let services = self.services.config();
        let root = self.root();
        self.service
            .refresh(&self.owner, &root, &services, self.sources())
            .await?;
        let mut guard = self.guard.lock().await;
        self.service
            .publish_latest(
                &BaselineScope {
                    owner: &self.owner,
                    actor,
                    root: &root,
                    services: &services,
                    sources: self.sources(),
                },
                &mut guard,
            )
            .await
    }
}

#[async_trait]
impl InventoryPublisher for InventoryRefresher {
    async fn publish_latest(
        &self,
        actor: &UserId,
    ) -> std::result::Result<usize, InventoryPublishError> {
        let outcomes = Self::publish_latest(self, actor)
            .await
            .map_err(|error| InventoryPublishError::Failed(Box::new(error)))?;
        Ok(outcomes
            .iter()
            .filter(|outcome| outcome.status == LatestPublicationStatus::Published)
            .count())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MemberRefresher(pub(crate) Arc<InventoryRefresher>);
