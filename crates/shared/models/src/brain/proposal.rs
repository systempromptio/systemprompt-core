//! Proposals produced by analyses: config diffs or fact actions that a
//! person approves or rejects; nothing self-applies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{BrainProposalId, UserId};

/// What a proposal would change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalKind {
    ConfigDiff,
    FactActions,
}

/// Where a proposal stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Open,
    Approved,
    Rejected,
}

/// A proposal an analysis drafts, before it is stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalDraft {
    pub analysis: String,
    pub kind: ProposalKind,
    pub summary: String,
    // JSON: the proposed diff or action list is analysis-specific.
    pub body: serde_json::Value,
}

/// A stored proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainProposal {
    pub id: BrainProposalId,
    pub analysis: String,
    pub kind: ProposalKind,
    pub status: ProposalStatus,
    pub summary: String,
    // JSON: the proposed diff or action list is analysis-specific.
    pub body: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub decided_by: Option<UserId>,
    pub decided_at: Option<DateTime<Utc>>,
}
