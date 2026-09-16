//! The provenance one automatic publication pass carries: which composed
//! services tree it ran against, which kit owns the resource, and which
//! inventory generation observed it.
//!
//! The publication itself records only a digest, which says the bytes
//! changed but not where they came from. Stamping the composed hash, the
//! owning bundle's content hash and the inventory generation into the
//! comparison evidence is what lets a reader walk a published revision back
//! to the declaration that produced it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::ResourceRevisionId;

use super::InventoryEntry;
use super::publish_latest::Managed;
use crate::managed::{
    AssetDigest, ComparisonEvidence, INVENTORY_REFRESH_SOURCE, PublicationAction,
    PublicationRequest,
};

// Why: captured once per pass rather than per entry — the composed tree and the
// bundle ownership map are the same for every publication the pass creates, and
// reading them per entry would hash the services tree on every skill.
pub(super) struct PassProvenance {
    pub(super) composed_hash: Option<String>,
    pub(super) inventory_generation: i64,
    pub(super) bundle_hashes: BTreeMap<String, String>,
}

pub(super) fn inventory_request(
    entry: &InventoryEntry,
    managed: &Managed,
    revision: &ResourceRevisionId,
    tree: &AssetDigest,
    provenance: &PassProvenance,
) -> PublicationRequest {
    PublicationRequest {
        resource_id: managed.resource_id.clone(),
        revision_id: Some(revision.clone()),
        action: if managed.generation == 0 {
            PublicationAction::InitialAdoption
        } else {
            PublicationAction::PublishImprovement
        },
        expected_generation: managed.generation,
        operation_key: format!("inventory-sync:{}:{revision}", entry.resource_key),
        comparison_evidence: ComparisonEvidence {
            experiment_id: None,
            recorded: BTreeMap::from([
                (
                    "source".to_owned(),
                    serde_json::json!(INVENTORY_REFRESH_SOURCE),
                ),
                (
                    "previous_revision".to_owned(),
                    serde_json::json!(managed.published),
                ),
                ("digest".to_owned(), serde_json::json!(tree)),
                (
                    "composed_hash".to_owned(),
                    serde_json::json!(provenance.composed_hash),
                ),
                (
                    "bundle_content_hash".to_owned(),
                    serde_json::json!(provenance.bundle_hashes.get(&entry.resource_key)),
                ),
                (
                    "inventory_generation".to_owned(),
                    serde_json::json!(provenance.inventory_generation),
                ),
            ]),
        },
        limitations: "Published automatically from the configured services tree".to_owned(),
    }
}
