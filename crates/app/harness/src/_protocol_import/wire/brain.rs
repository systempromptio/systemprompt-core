//! The Brain: an organisation's revisioned facts under `/api/v1/brain`, the
//! proposals its analyses draft, and the memory headers of a gateway call.
//!
//! Facts, revisions, proposals, scopes, statuses and subjects are
//! `systemprompt-models`' own types and the ids are
//! `systemprompt-identifiers`', both re-exported here. This module adds the
//! route bodies those crates lack, field for field as the instance writes
//! them: answers tolerate unknown fields, requests carry exactly the fields the
//! instance accepts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
pub use systemprompt_identifiers::{
    BrainDisputeId, BrainFactId, BrainFactRevisionId, BrainProposalId, EventOutboxId, FactKindId,
    PipelineVersion, TopicId,
};
pub use systemprompt_models::brain::{
    BrainFact, BrainProposal, EnteredFact, EnteredStatus, FactRef, FactRevision, FactScope,
    FactStatus, ProposalKind, ProposalStatus, RevisionChange, ScopeLevel, SubjectKey,
    SubjectKeyError, WithheldReason,
};

pub const MEMORY_OFF: &str = "off";

/// One page of a fact listing and the cursor of the next page, `None` when
/// the listing ended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactPage {
    pub facts: Vec<BrainFact>,
    #[serde(default)]
    pub next: Option<String>,
}

/// The terms of one fact's in-context importance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImportanceTerms {
    pub base: f64,
    pub subject: f64,
    pub workflow: f64,
    pub recency: f64,
    pub usage: f64,
    pub total: f64,
}

/// Why a fact ranks where it does for the caller, and the reason recall
/// would withhold it, if any.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactWhy {
    pub fact: BrainFact,
    pub terms: ImportanceTerms,
    #[serde(default)]
    pub withheld: Option<WithheldReason>,
}

/// The answer to an entered fact: the outbox event the instance builds the
/// fact from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnterAccepted {
    pub outbox_id: EventOutboxId,
}

/// The body of a dispute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisputeRequest {
    pub reason: String,
}

/// The answer to a dispute: the dispute it opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisputeCreated {
    pub dispute_id: BrainDisputeId,
}

/// The body of a retraction; an empty reason gives none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetractRequest {
    pub reason: String,
}

/// The body of a field edit, merged onto the stored fields: a key sent as
/// `null` removes an optional field, a key left out is kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldsRequest {
    // JSON: fact fields are typed by the fact's kind declaration, which only the instance holds.
    pub fields: serde_json::Map<String, serde_json::Value>,
}

/// How a dispute is decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisputeOutcome {
    Upheld,
    Dismissed,
}

/// The body of a dispute resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveRequest {
    pub fact_id: BrainFactId,
    pub outcome: DisputeOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The window a proposal's dry-run replay processes again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRequest {
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
}

/// What a replay would do to one kind's facts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeCounts {
    pub create: u64,
    pub revise: u64,
    pub dispute: u64,
    pub unchanged: u64,
}

/// A proposal's dry-run replay: the events processed again under a pipeline
/// version and what each fact write would have changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayReport {
    pub pipeline_version: PipelineVersion,
    pub events: u64,
    pub failed: u64,
    pub undecodable: u64,
    pub truncated: bool,
    pub would_create: u64,
    pub would_revise: u64,
    pub would_dispute: u64,
    pub unchanged: u64,
    pub tokens: u64,
    pub per_kind: BTreeMap<FactKindId, ChangeCounts>,
}

/// Why an `x-systemprompt-memory-ids` value does not read as
/// `<fact id>@<revision>` entries.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MemoryIdsError {
    #[error("memory id entry {position} is not <fact id>@<revision>")]
    Shape { position: usize },
    #[error("memory id entry {position} names a malformed fact id")]
    FactId {
        position: usize,
        #[source]
        source: systemprompt_identifiers::error::IdValidationError,
    },
    #[error("memory id entry {position} names a malformed revision")]
    Revision {
        position: usize,
        #[source]
        source: std::num::ParseIntError,
    },
}

pub fn parse_memory_ids(value: &str) -> Result<Vec<FactRef>, MemoryIdsError> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .enumerate()
        .map(|(position, entry)| {
            let (id, revision) = entry
                .rsplit_once('@')
                .ok_or(MemoryIdsError::Shape { position })?;
            let id = BrainFactId::try_new(id)
                .map_err(|source| MemoryIdsError::FactId { position, source })?;
            let revision = revision
                .parse::<i32>()
                .map_err(|source| MemoryIdsError::Revision { position, source })?;
            Ok(FactRef { id, revision })
        })
        .collect()
}
