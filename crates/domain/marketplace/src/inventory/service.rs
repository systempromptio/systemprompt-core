//! Application-injected catalog observations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{InventoryStatus, SourcesContext, scan_configured_inventory};
use crate::managed::{ManagedError, ManagedRepository, Result};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::InstallServicesView;

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
        view: &InstallServicesView,
    ) -> Result<InventoryStatus> {
        let root = view.root().to_path_buf();
        let services = view.config();
        let sources = SourcesContext::of(view);
        let configured =
            tokio::task::spawn_blocking(move || scan_configured_inventory(&root, &services))
                .await
                .map_err(|error| ManagedError::Internal {
                    context: "inventory scan task",
                    source: Box::new(error),
                })
                .and_then(std::convert::identity);
        let result = match configured {
            Ok(configured) => {
                self.repository
                    .reconcile_inventory(owner, &configured, sources)
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
