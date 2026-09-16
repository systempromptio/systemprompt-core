//! Reconciliation requests, recorded conflicts and the three-way conflict
//! detection that seeds them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ManagedReconciliationId, ManagedResourceId, ResourceRevisionId};

use super::super::{AssetDigest, RevisionFiles};
use super::merge::same_file;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "the request names four distinct revisions; the `_id` suffix is the typed-identifier convention"
)]
pub struct ReconciliationRequest {
    pub resource_id: ManagedResourceId,
    pub upstream_base_revision_id: ResourceRevisionId,
    pub managed_candidate_revision_id: ResourceRevisionId,
    pub incoming_revision_id: ResourceRevisionId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationConflict {
    pub path: String,
    pub base_digest: Option<AssetDigest>,
    pub candidate_digest: Option<AssetDigest>,
    pub incoming_digest: Option<AssetDigest>,
    pub resolution: Option<ConflictResolution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum ReconciliationStatus {
    Open,
    Resolved,
    WithdrawalProposed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum ConflictResolution {
    Candidate,
    Incoming,
    Manual,
    Delete,
}

impl ConflictResolution {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Incoming => "incoming",
            Self::Manual => "manual",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationRecord {
    pub id: ManagedReconciliationId,
    pub status: ReconciliationStatus,
    pub conflicts: Vec<ReconciliationConflict>,
    pub resolved_revision_id: Option<ResourceRevisionId>,
}

#[derive(Debug, Clone, Copy)]
pub struct ConflictDecision<'a> {
    pub path: &'a str,
    pub resolution: ConflictResolution,
    pub resolved_digest: Option<&'a str>,
}

pub(super) fn detect_conflicts(
    base: &RevisionFiles,
    candidate: &RevisionFiles,
    incoming: &RevisionFiles,
) -> Vec<ReconciliationConflict> {
    let mut paths = std::collections::BTreeSet::new();
    paths.extend(base.0.keys().cloned());
    paths.extend(candidate.0.keys().cloned());
    paths.extend(incoming.0.keys().cloned());
    let digest = |files: &RevisionFiles, path: &str| {
        files.0.get(path).map(|file| AssetDigest::of(&file.bytes))
    };
    paths
        .into_iter()
        .filter(|path| {
            !same_file(candidate.0.get(path), base.0.get(path))
                && !same_file(incoming.0.get(path), base.0.get(path))
                && !same_file(candidate.0.get(path), incoming.0.get(path))
        })
        .map(|path| ReconciliationConflict {
            base_digest: digest(base, &path),
            candidate_digest: digest(candidate, &path),
            incoming_digest: digest(incoming, &path),
            path,
            resolution: None,
        })
        .collect()
}
