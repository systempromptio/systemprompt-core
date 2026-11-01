//! Capture-side ledger rows: extractions, per-event results, failures and
//! disputes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{BrainFactId, EventOutboxId, FactKindId, PipelineVersion};
use systemprompt_models::brain::{ExtractionRecord, FactCandidate};

/// One extraction call to record.
#[derive(Debug, Clone, PartialEq)]
pub struct NewExtraction {
    pub outbox_id: EventOutboxId,
    pub pipeline_version: PipelineVersion,
    pub kind: FactKindId,
    pub record: ExtractionRecord,
    pub candidates: u32,
}

/// How the pipeline finished with one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventOutcome {
    Processed,
    Ignored,
    NoFilter,
    Rejected,
    Failed,
    Abandoned,
}

impl EventOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Processed => "processed",
            Self::Ignored => "ignored",
            Self::NoFilter => "no_filter",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
            Self::Abandoned => "abandoned",
        }
    }
}

/// The finished result of one event under one pipeline version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventResult {
    pub outbox_id: EventOutboxId,
    pub pipeline_version: PipelineVersion,
    pub outcome: EventOutcome,
    pub filter: Option<String>,
    pub facts_written: u32,
}

/// A failed pipeline step for one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFailure {
    pub outbox_id: EventOutboxId,
    pub pipeline_version: Option<PipelineVersion>,
    pub stage: &'static str,
    pub reason: String,
    pub attempt: u32,
}

/// Whether a dispute still needs a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisputeStatus {
    Open,
    Resolved,
}

impl DisputeStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
        }
    }
}

/// A conflicting candidate raised against an existing fact.
#[derive(Debug, Clone, PartialEq)]
pub struct NewDispute {
    pub fact_id: BrainFactId,
    pub candidate: FactCandidate,
    pub similarity: Option<f64>,
    pub reason: String,
}
