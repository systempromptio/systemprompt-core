//! Inventory identities remain independent of naming and usage evidence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use systemprompt_identifiers::{
    InventoryEntryId, ManagedReconciliationId, ManagedResourceId, ManagedSourceId,
    ResourceRevisionId, UserId,
};
use systemprompt_loader::bundle::BundleSources;
use systemprompt_manifest::services::InstallServicesView;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_models::feedback::inventory::{InventoryAvailability, InventoryOrigin};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct InventoryEntry {
    pub entry_id: InventoryEntryId,
    pub kind: String,
    pub resource_key: String,
    pub origin: InventoryOrigin,
    pub configured_key: Option<String>,
    pub resource_id: Option<ManagedResourceId>,
    pub source_id: Option<ManagedSourceId>,
    pub availability: InventoryAvailability,
    pub latest_revision_id: Option<ResourceRevisionId>,
    pub published_revision_id: Option<ResourceRevisionId>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfiguredInventoryEntry {
    pub kind: String,
    pub resource_key: String,
    pub relative_root: String,
    pub availability: InventoryAvailability,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InventoryStatus {
    pub generation: i64,
    pub observed_at: Option<DateTime<Utc>>,
    pub entries: i64,
    pub last_error: Option<String>,
}

pub fn configured_identity(owner: &UserId, kind: &str, key: &str) -> InventoryEntryId {
    InventoryEntryId::new(
        crate::managed::AssetDigest::of(format!("configured/{owner}/{kind}/{key}").as_bytes())
            .as_str(),
    )
}

pub(super) fn managed_identity(resource: &ManagedResourceId) -> InventoryEntryId {
    InventoryEntryId::new(format!("managed-{resource}"))
}

/// What an inventory pass reads about the services tree's sources: the
/// bundle sources and the active root, both from the read-only
/// [`InstallServicesView`] an install member also reads.
#[derive(Debug, Clone)]
pub struct SourcesContext<'a> {
    pub bundles: BundleSources,
    pub active_root: &'a ActiveServicesRoot,
}

impl<'a> SourcesContext<'a> {
    #[must_use]
    pub fn of(view: &'a InstallServicesView) -> Self {
        Self {
            bundles: BundleSources::of(view),
            active_root: view.active_root(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BaselineScope<'a> {
    pub owner: &'a UserId,
    pub actor: &'a UserId,
    pub view: &'a InstallServicesView,
}

impl<'a> BaselineScope<'a> {
    #[must_use]
    pub fn root(&self) -> &'a Path {
        self.view.root()
    }

    #[must_use]
    pub fn sources(&self) -> SourcesContext<'a> {
        SourcesContext::of(self.view)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InventoryReconciliation {
    pub id: ManagedReconciliationId,
    pub status: crate::managed::ReconciliationStatus,
    pub upstream_base_revision_id: ResourceRevisionId,
    pub managed_candidate_revision_id: ResourceRevisionId,
    pub incoming_revision_id: ResourceRevisionId,
    pub resolved_revision_id: Option<ResourceRevisionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ManagedSourceKind {
    Git,
    LocalTree,
    Managed,
}

#[derive(Debug, Clone)]
pub(super) struct InventoryResource {
    pub(super) id: ManagedResourceId,
    pub(super) source_id: ManagedSourceId,
    pub(super) kind: String,
    pub(super) resource_key: String,
    pub(super) source_kind: ManagedSourceKind,
    pub(super) publication_state: Option<String>,
    pub(super) published_revision: Option<ResourceRevisionId>,
    pub(super) latest_revision: Option<ResourceRevisionId>,
    pub(super) bound_entry: Option<InventoryEntryId>,
    pub(super) bound_path: Option<String>,
    pub(super) upstream_removed: bool,
    pub(super) open_reconciliation: bool,
}
