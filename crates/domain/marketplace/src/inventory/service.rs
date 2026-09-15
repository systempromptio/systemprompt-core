//! Application-injected catalog observations and bounded baseline preparation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::catalog::invalid;
use super::{BaselineCapture, BaselineScope, InventoryStatus, scan_configured_inventory};
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

    pub async fn prepare_baselines(
        &self,
        scope: &BaselineScope<'_>,
        request: &super::BaselinePreparation,
    ) -> Result<Vec<BaselineCapture>> {
        let owner = scope.owner;
        let entries = self
            .repository
            .inventory(owner, request.after.as_ref(), request.limit)
            .await?;
        let mut outcomes = Vec::new();
        for entry in entries {
            if let Some(previous) = self
                .repository
                .inventory_capture(owner, &entry.entry_id, &request.operation_id)
                .await?
            {
                outcomes.push(previous);
                continue;
            }
            let capture = match self.capture_entry(scope, &entry).await {
                Ok((revision, reconciliation)) => BaselineCapture {
                    entry_id: entry.entry_id,
                    operation_id: request.operation_id.clone(),
                    status: if reconciliation.is_some() {
                        "reconciliation_required"
                    } else {
                        "ready"
                    }
                    .to_owned(),
                    revision_id: Some(revision),
                    reconciliation_id: reconciliation,
                    diagnostic: None,
                },
                Err(error) => BaselineCapture {
                    entry_id: entry.entry_id,
                    operation_id: request.operation_id.clone(),
                    status: "blocked".to_owned(),
                    revision_id: None,
                    reconciliation_id: None,
                    diagnostic: Some(error.to_string().chars().take(2000).collect()),
                },
            };
            self.repository
                .record_inventory_capture(owner, &capture)
                .await?;
            outcomes.push(
                self.repository
                    .inventory_capture(owner, &capture.entry_id, &request.operation_id)
                    .await?
                    .ok_or_else(|| invalid("Recorded baseline outcome disappeared"))?,
            );
        }
        self.refresh(owner, scope.root, scope.services).await?;
        Ok(outcomes)
    }
}
