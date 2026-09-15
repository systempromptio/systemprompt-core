//! Automatic publication of the configured services tree: every available
//! configured skill is captured and its latest revision published, retaining
//! every earlier generation in publication history.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::baseline::configured_files;
use super::{BaselineScope, InventoryEntry, InventoryService};
use crate::managed::{
    AssetDigest, INVENTORY_REFRESH_SOURCE, ManagedResolution, PublicationAction,
    PublicationAdmission, PublicationRequest, ResourceKind, Result,
};
use serde::Serialize;
use std::collections::HashMap;
use systemprompt_identifiers::{InventoryEntryId, ManagedResourceId, ResourceRevisionId};
use systemprompt_models::feedback::inventory::{InventoryAvailability, InventoryOrigin};

const PAGE_SIZE: u32 = 100;

/// Outcome of one inventory entry in a `publish_latest` pass.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct LatestPublication {
    pub entry_id: InventoryEntryId,
    pub resource_key: String,
    pub status: LatestPublicationStatus,
    pub revision_id: Option<ResourceRevisionId>,
    pub generation: Option<i64>,
    pub diagnostic: Option<String>,
}

/// What `publish_latest` did with one configured skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LatestPublicationStatus {
    Unchanged,
    Published,
    Withdrawn,
    ReconciliationRequired,
    Blocked,
}

/// Per-process memo of the configured tree digest last seen for each entry, so
/// a repeated pass over an unchanged tree skips retained-revision lookups.
#[derive(Debug, Default)]
pub struct PublishGuard {
    seen: HashMap<InventoryEntryId, AssetDigest>,
}

struct Managed {
    resource_id: ManagedResourceId,
    generation: i64,
    published: Option<ResourceRevisionId>,
}

impl InventoryService {
    /// Publishes the latest configured revision of every available skill,
    /// then refreshes the inventory so published-revision projections are
    /// current. Withdrawn resources stay withdrawn; open reconciliations are
    /// reported and never published over.
    pub async fn publish_latest(
        &self,
        scope: &BaselineScope<'_>,
        guard: &mut PublishGuard,
    ) -> Result<Vec<LatestPublication>> {
        let owner = scope.owner;
        let mut outcomes = Vec::new();
        let mut after: Option<InventoryEntryId> = None;
        loop {
            let page = self
                .repository
                .inventory(owner, after.as_ref(), PAGE_SIZE)
                .await?;
            let Some(last) = page.last().map(|entry| entry.entry_id.clone()) else {
                break;
            };
            let page_len = page.len();
            for entry in page {
                if entry.kind != "skill" || entry.origin != InventoryOrigin::Configured {
                    continue;
                }
                outcomes.push(self.publish_entry(scope, guard, &entry).await?);
            }
            if page_len < PAGE_SIZE as usize {
                break;
            }
            after = Some(last);
        }
        self.refresh(owner, scope.root, scope.services).await?;
        Ok(outcomes)
    }

    async fn publish_entry(
        &self,
        scope: &BaselineScope<'_>,
        guard: &mut PublishGuard,
        entry: &InventoryEntry,
    ) -> Result<LatestPublication> {
        let owner = scope.owner;
        match entry.availability {
            InventoryAvailability::Available => {},
            InventoryAvailability::Withdrawn => {
                return Ok(outcome(entry, LatestPublicationStatus::Withdrawn));
            },
            InventoryAvailability::Conflicting => {
                let status = if self.has_open_reconciliation(scope, entry).await? {
                    LatestPublicationStatus::ReconciliationRequired
                } else {
                    LatestPublicationStatus::Blocked
                };
                return Ok(outcome(entry, status));
            },
            InventoryAvailability::Unavailable => {
                return Ok(outcome(entry, LatestPublicationStatus::Blocked));
            },
        }
        let canonical = std::fs::canonicalize(scope.root)?;
        let tree = match configured_files(&canonical, entry, scope.services) {
            Ok(files) => AssetDigest::of(&serde_jcs::to_vec(&files)?),
            Err(error) => return Ok(blocked(entry, &error)),
        };
        if guard.seen.get(&entry.entry_id) == Some(&tree) && entry.published_revision_id.is_some() {
            return Ok(outcome(entry, LatestPublicationStatus::Unchanged));
        }
        if let ManagedResolution::Withdrawn { .. } = self
            .repository
            .resolve_managed(owner, ResourceKind::Skill, &entry.resource_key)
            .await?
        {
            guard.seen.insert(entry.entry_id.clone(), tree);
            return Ok(outcome(entry, LatestPublicationStatus::Withdrawn));
        }
        let (captured, reconciliation) = match self.capture_entry(scope, entry).await {
            Ok(captured) => captured,
            Err(error) => return Ok(blocked(entry, &error)),
        };
        if reconciliation.is_some() {
            return Ok(outcome(
                entry,
                LatestPublicationStatus::ReconciliationRequired,
            ));
        }
        // Why: capture reports the published revision whenever the authoring
        // head already matched the tree, and binds a resource for an entry that
        // had none; the head and the managed resolution are authoritative only
        // after it has run.
        let revision = self
            .repository
            .inventory_authoring_head(owner, &entry.entry_id)
            .await?
            .unwrap_or(captured);
        let managed = self.bound_resource(scope, entry).await?;
        if managed.published.as_ref() == Some(&revision) {
            guard.seen.insert(entry.entry_id.clone(), tree);
            return Ok(LatestPublication {
                revision_id: Some(revision),
                generation: Some(managed.generation),
                ..outcome(entry, LatestPublicationStatus::Unchanged)
            });
        }
        let request = PublicationRequest {
            resource_id: managed.resource_id,
            revision_id: Some(revision.clone()),
            action: if managed.generation == 0 {
                PublicationAction::InitialAdoption
            } else {
                PublicationAction::PublishImprovement
            },
            expected_generation: managed.generation,
            operation_key: format!("inventory-sync:{}:{revision}", entry.resource_key),
            comparison_evidence: serde_json::json!({
                "source": INVENTORY_REFRESH_SOURCE,
                "previous_revision": managed.published,
                "digest": tree,
            }),
            limitations: "Published automatically from the configured services tree".to_owned(),
        };
        let decision = self
            .repository
            .publish_with_admission(
                owner,
                scope.actor,
                &request,
                PublicationAdmission::InventorySync,
            )
            .await?;
        guard.seen.insert(entry.entry_id.clone(), tree);
        Ok(LatestPublication {
            revision_id: Some(revision),
            generation: Some(decision.generation),
            ..outcome(entry, LatestPublicationStatus::Published)
        })
    }

    async fn bound_resource(
        &self,
        scope: &BaselineScope<'_>,
        entry: &InventoryEntry,
    ) -> Result<Managed> {
        match self
            .repository
            .resolve_managed(scope.owner, ResourceKind::Skill, &entry.resource_key)
            .await?
        {
            ManagedResolution::NeverAdopted { resource_id } => Ok(Managed {
                resource_id,
                generation: 0,
                published: None,
            }),
            ManagedResolution::Published {
                resource_id,
                generation,
                revision_id,
                ..
            } => Ok(Managed {
                resource_id,
                generation,
                published: Some(revision_id),
            }),
            _ => Err(crate::managed::ManagedError::Unavailable),
        }
    }

    async fn has_open_reconciliation(
        &self,
        scope: &BaselineScope<'_>,
        entry: &InventoryEntry,
    ) -> Result<bool> {
        if entry.resource_id.is_none() {
            return Ok(false);
        }
        Ok(self
            .repository
            .inventory_reconciliations(scope.owner, &entry.entry_id, None)
            .await?
            .iter()
            .any(|record| record.status == "open"))
    }
}

fn outcome(entry: &InventoryEntry, status: LatestPublicationStatus) -> LatestPublication {
    LatestPublication {
        entry_id: entry.entry_id.clone(),
        resource_key: entry.resource_key.clone(),
        status,
        revision_id: entry.published_revision_id.clone(),
        generation: None,
        diagnostic: entry.diagnostic.clone(),
    }
}

fn blocked(entry: &InventoryEntry, error: &crate::managed::ManagedError) -> LatestPublication {
    LatestPublication {
        diagnostic: Some(error.to_string().chars().take(2000).collect()),
        ..outcome(entry, LatestPublicationStatus::Blocked)
    }
}
