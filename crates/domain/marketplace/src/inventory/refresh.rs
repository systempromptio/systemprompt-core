//! [`InventoryRefresher`]: the configured-owner inventory refresh, bound to
//! the services tree and the profile's bundle sources.
//!
//! The `managed_resources` member builds one refresher in its provide fn
//! from the provided repository and publish guard, the sealed system admin
//! and the read-only [`InstallServicesView`] kernel seeds, and provides it
//! sealed. The member's
//! `managed_inventory_refresh` job resolves it because a job context keeps
//! the sealed values its own member provided, and calls only
//! [`InventoryRefresher::fingerprint`] and
//! [`InventoryRefresher::publish_as_owner`]; another member's job, command,
//! route or runner context resolves neither it nor a publish under a
//! caller-chosen actor. A kernel route reaches a refresher only as the sealed
//! `DynInventoryPublisher` (`publish_latest` with the authenticated request's
//! user as the actor), a separate instance. Neither reads the
//! profile, the configuration or a secret: everything a refresh reads is the
//! [`InstallServicesView`], the same view an install member reads to call
//! [`InventoryService::refresh`] itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::InstallServicesView;
use systemprompt_traits::{InventoryPublishError, InventoryPublisher};
use tokio::sync::Mutex;

use super::{
    BaselineScope, InventoryService, InventoryStatus, LatestPublication, LatestPublicationStatus,
    PublishGuard, configured_inventory_fingerprint,
};
use crate::managed::{ManagedRepository, Result};

/// The handles an [`InventoryRefresher`] is built from.
#[derive(Debug)]
pub struct InventoryRefresherParts {
    pub repository: Arc<ManagedRepository>,
    pub guard: Arc<Mutex<PublishGuard>>,
    pub services: InstallServicesView,
    pub owner: UserId,
}

/// Reconciles and publishes the configured owner's inventory.
#[derive(Debug)]
pub struct InventoryRefresher {
    service: InventoryService,
    guard: Arc<Mutex<PublishGuard>>,
    services: InstallServicesView,
    owner: UserId,
}

impl InventoryRefresher {
    #[must_use]
    pub fn new(parts: InventoryRefresherParts) -> Self {
        Self {
            service: InventoryService::new(parts.repository.as_ref().clone()),
            guard: parts.guard,
            services: parts.services,
            owner: parts.owner,
        }
    }

    #[must_use]
    pub const fn owner(&self) -> &UserId {
        &self.owner
    }

    pub fn fingerprint(&self) -> Result<[u8; 32]> {
        configured_inventory_fingerprint(self.services.root(), &self.services.config())
    }

    pub async fn refresh(&self) -> Result<InventoryStatus> {
        self.service.refresh(&self.owner, &self.services).await
    }

    pub async fn publish_as_owner(&self) -> Result<Vec<LatestPublication>> {
        let owner = self.owner.clone();
        self.publish_latest(&owner).await
    }

    pub async fn publish_latest(&self, actor: &UserId) -> Result<Vec<LatestPublication>> {
        self.service.refresh(&self.owner, &self.services).await?;
        let mut guard = self.guard.lock().await;
        self.service
            .publish_latest(
                &BaselineScope {
                    owner: &self.owner,
                    actor,
                    view: &self.services,
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
