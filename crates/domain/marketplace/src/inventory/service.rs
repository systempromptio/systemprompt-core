//! Application-injected catalog observations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{InventoryStatus, scan_configured_inventory};
use crate::managed::{ManagedError, ManagedRepository, Result};
use std::path::Path;
use systemprompt_identifiers::UserId;
use systemprompt_models::services::ServicesConfig;

#[derive(Debug, Clone)]
pub struct InventoryService {
    pub(super) repository: ManagedRepository,
}

impl InventoryService {
    pub const fn new(repository: ManagedRepository) -> Self {
        Self { repository }
    }

    pub async fn refresh(
        &self,
        owner: &UserId,
        root: &Path,
        services: &ServicesConfig,
    ) -> Result<InventoryStatus> {
        let root = root.to_path_buf();
        let services = services.clone();
        let configured =
            tokio::task::spawn_blocking(move || scan_configured_inventory(&root, &services))
                .await
                .map_err(|error| {
                    ManagedError::Invalid(format!("Inventory scan task failed: {error}"))
                })
                .and_then(std::convert::identity);
        let result = match configured {
            Ok(configured) => {
                self.repository
                    .reconcile_inventory(owner, &configured)
                    .await
            },
            Err(error) => Err(error),
        };
        if result.is_err() {
            self.repository.record_inventory_failure(owner).await?;
        }
        result
    }
}
