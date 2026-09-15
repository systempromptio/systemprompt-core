//! Reviewed publication and generation-pinned managed resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    EvalExperimentId, EventOutboxId, ManagedResourceId, PublicationId, PublicationReviewId,
    ResourceRevisionId, UserId,
};

use super::error::invalid;
use super::{AssetDigest, ManagedError, ManagedRepository, ResourceKind, Result, RevisionBundle};

mod history;
mod repository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublicationAction {
    InitialAdoption,
    PublishImprovement,
    Withdraw,
    Rollback,
}

impl PublicationAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InitialAdoption => "initial_adoption",
            Self::PublishImprovement => "publish_improvement",
            Self::Withdraw => "withdraw",
            Self::Rollback => "rollback",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "initial_adoption" => Ok(Self::InitialAdoption),
            "publish_improvement" => Ok(Self::PublishImprovement),
            "withdraw" => Ok(Self::Withdraw),
            "rollback" => Ok(Self::Rollback),
            _ => Err(ManagedError::Integrity),
        }
    }
}

/// How a publication request earned its way past review.
///
/// `Attested` is the reviewed path: an improvement must carry an evaluation
/// attestation. `InventorySync` is the configured-tree path: the services tree
/// on disk is the reviewed artefact, so evidence names `inventory_refresh` as
/// its source instead of an experiment, and only forward actions are admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub enum PublicationAdmission {
    /// Reviewed publication backed by an evaluation attestation.
    Attested,
    /// Automatic publication of the configured services tree.
    InventorySync,
}

/// Evidence `source` value an inventory-sync publication must carry.
pub const INVENTORY_REFRESH_SOURCE: &str = "inventory_refresh";

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicationRequest {
    pub resource_id: ManagedResourceId,
    pub revision_id: Option<ResourceRevisionId>,
    pub action: PublicationAction,
    pub expected_generation: i64,
    pub operation_key: String,
    pub comparison_evidence: ComparisonEvidence,
    pub limitations: String,
}

/// The reviewer's evidence for a publication. `PublishImprovement` requires
/// `experiment_id` naming an attested experiment; everything else the
/// reviewer attaches is retained verbatim with the review.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ComparisonEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment_id: Option<EvalExperimentId>,
    #[serde(flatten)]
    // JSON: reviewer-attached evidence is retained verbatim, never interpreted
    pub recorded: BTreeMap<String, serde_json::Value>,
}

impl ComparisonEvidence {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.experiment_id.is_none() && self.recorded.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PublicationDecision {
    pub publication_id: PublicationId,
    pub review_id: PublicationReviewId,
    pub resource_id: ManagedResourceId,
    pub generation: i64,
    pub action: PublicationAction,
    pub revision_id: Option<ResourceRevisionId>,
    pub bundle_digest: Option<AssetDigest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ManagedResolution {
    NotManaged,
    NeverAdopted {
        resource_id: ManagedResourceId,
    },
    Published {
        publication_id: PublicationId,
        resource_id: ManagedResourceId,
        generation: i64,
        revision_id: ResourceRevisionId,
        bundle_digest: AssetDigest,
    },
    Withdrawn {
        publication_id: PublicationId,
        resource_id: ManagedResourceId,
        generation: i64,
    },
    IntegrityFailure {
        resource_id: ManagedResourceId,
        generation: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PublicationHistoryEntry {
    pub decision: PublicationDecision,
    pub distributed: bool,
    pub installation_verified: bool,
    pub reviewer_id: UserId,
    pub comparison_evidence: ComparisonEvidence,
    pub limitations: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

fn validate_request(request: &PublicationRequest) -> Result<()> {
    if request.expected_generation < 0
        || request.operation_key.trim().is_empty()
        || request.operation_key.len() > 200
        || request.limitations.len() > 4000
        || serde_jcs::to_vec(&request.comparison_evidence)?.len() > 65_536
        || (request.action == PublicationAction::PublishImprovement
            && request.comparison_evidence.is_empty())
    {
        return Err(invalid("Invalid publication review input"));
    }
    let revision_required = request.action != PublicationAction::Withdraw;
    if revision_required != request.revision_id.is_some()
        || (request.action == PublicationAction::InitialAdoption
            && request.expected_generation != 0)
        || (request.action != PublicationAction::InitialAdoption
            && request.expected_generation == 0)
    {
        return Err(invalid(
            "Publication action does not match its generation or revision",
        ));
    }
    Ok(())
}

fn validate_admission(request: &PublicationRequest, admission: PublicationAdmission) -> Result<()> {
    if admission != PublicationAdmission::InventorySync {
        return Ok(());
    }
    if !matches!(
        request.action,
        PublicationAction::InitialAdoption | PublicationAction::PublishImprovement
    ) {
        return Err(ManagedError::Conflict(
            "Inventory synchronisation only adopts or advances configured content".to_owned(),
        ));
    }
    let source = request
        .comparison_evidence
        .recorded
        .get("source")
        .and_then(serde_json::Value::as_str);
    if source != Some(INVENTORY_REFRESH_SOURCE) {
        return Err(invalid(
            "Inventory synchronisation evidence must name the inventory refresh as its source",
        ));
    }
    Ok(())
}

fn request_digest(
    request: &PublicationRequest,
    reviewer: &UserId,
    bundle_digest: Option<&AssetDigest>,
) -> Result<AssetDigest> {
    Ok(AssetDigest::of(&serde_jcs::to_vec(&serde_json::json!({
        "request": request,
        "reviewer_id": reviewer,
        "bundle_digest": bundle_digest,
    }))?))
}

pub(super) struct PublicationRow {
    pub id: String,
    pub review_id: String,
    pub generation: i64,
    pub action: String,
    pub revision_id: Option<String>,
    pub bundle_digest: Option<String>,
}

fn decision_from_row(
    resource_id: &ManagedResourceId,
    row: PublicationRow,
) -> Result<PublicationDecision> {
    let digest = row.bundle_digest.map(AssetDigest::try_from).transpose()?;
    Ok(PublicationDecision {
        publication_id: PublicationId::new(row.id),
        review_id: PublicationReviewId::new(row.review_id),
        resource_id: resource_id.clone(),
        generation: row.generation,
        action: PublicationAction::parse(&row.action)?,
        revision_id: row.revision_id.map(ResourceRevisionId::new),
        bundle_digest: digest,
    })
}

pub(super) struct SelectionRow {
    pub generation: i64,
    pub state: String,
    pub publication_id: String,
    pub revision_id: Option<String>,
    pub bundle_digest: Option<String>,
}

fn resolution_from_row(
    resource_id: ManagedResourceId,
    row: SelectionRow,
) -> Result<ManagedResolution> {
    let SelectionRow {
        generation,
        state,
        publication_id,
        revision_id,
        bundle_digest,
    } = row;
    let publication_id = PublicationId::new(publication_id);
    match state.as_str() {
        "withdrawn" => Ok(ManagedResolution::Withdrawn {
            publication_id,
            resource_id,
            generation,
        }),
        "published" => Ok(ManagedResolution::Published {
            publication_id,
            resource_id,
            generation,
            revision_id: ResourceRevisionId::new(revision_id.ok_or(ManagedError::Integrity)?),
            bundle_digest: AssetDigest::try_from(bundle_digest.ok_or(ManagedError::Integrity)?)?,
        }),
        _ => Err(ManagedError::Integrity),
    }
}
