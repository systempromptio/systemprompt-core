//! Read-side values: the recall query and context, scored facts, the
//! injection plan and the audit of what was injected or withheld.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{BrainFactId, BrainRecallId, UserId, WorkflowId};

use super::fact::BrainFact;
use super::scope::ScopeLevel;
use crate::auth::RequestActor;
use crate::origin::ClientKind;

/// A retrieval tier, fused by reciprocal rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalTier {
    Pinned,
    Subject,
    Context,
    FullText,
    Extension,
}

/// Why a candidate fact did not reach the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithheldReason {
    Scope,
    Source,
    AgentCapped,
    Stale,
    ReviewDue,
    Quota,
    Budget,
    Unavailable,
}

impl WithheldReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scope => "scope",
            Self::Source => "source",
            Self::AgentCapped => "agent_capped",
            Self::Stale => "stale",
            Self::ReviewDue => "review_due",
            Self::Quota => "quota",
            Self::Budget => "budget",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Who is asking, from where, at what time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallCtx {
    pub user_id: UserId,
    pub actor: RequestActor,
    pub client: ClientKind,
    pub project: Option<String>,
    pub workflow: Option<WorkflowId>,
    pub scopes: Vec<ScopeLevel>,
    pub now: DateTime<Utc>,
}

impl RecallCtx {
    #[must_use]
    pub fn new(user_id: UserId, actor: RequestActor, now: DateTime<Utc>) -> Self {
        Self {
            user_id,
            actor,
            client: ClientKind::Other,
            project: None,
            workflow: None,
            scopes: ScopeLevel::ALL.to_vec(),
            now,
        }
    }

    #[must_use]
    pub const fn with_client(mut self, client: ClientKind) -> Self {
        self.client = client;
        self
    }

    #[must_use]
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    #[must_use]
    pub fn with_workflow(mut self, workflow: WorkflowId) -> Self {
        self.workflow = Some(workflow);
        self
    }

    #[must_use]
    pub fn with_scopes(mut self, scopes: Vec<ScopeLevel>) -> Self {
        self.scopes = scopes;
        self
    }
}

/// What to look for: subject hints from the last user turn and how many
/// facts each tier may return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallQuery {
    pub subject_hints: String,
    pub limit: u32,
}

/// A fact's rank in one tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierRank {
    pub tier: RetrievalTier,
    pub rank: u32,
}

/// A retrieved fact with its fused score and the ranks behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredFact {
    pub fact: BrainFact,
    pub score: f64,
    pub ranks: Vec<TierRank>,
}

/// A fact that was not injected, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithheldFact {
    pub id: BrainFactId,
    pub reason: WithheldReason,
}

/// The facts chosen for one request within its token budget.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InjectionPlan {
    pub facts: Vec<ScoredFact>,
    pub withheld: Vec<WithheldFact>,
    pub tokens: u32,
}

/// A fact at the revision that was injected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactRef {
    pub id: BrainFactId,
    pub revision: i32,
}

/// The block a conversation's last injecting recall sent and the facts,
/// at their revisions, it named; same-turn reuse resends it unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredBlock {
    pub rendered: String,
    pub as_of: DateTime<Utc>,
    pub facts: Vec<FactRef>,
}

/// The audit of one recall: what was injected, what was withheld by
/// reason, the tokens spent and the instant the facts were read at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallAudit {
    pub recall_id: BrainRecallId,
    pub injected: Vec<FactRef>,
    pub withheld: BTreeMap<WithheldReason, u32>,
    pub tokens: u32,
    pub as_of: DateTime<Utc>,
}

impl RecallAudit {
    #[must_use]
    pub fn withheld_total(&self) -> u32 {
        self.withheld.values().sum()
    }
}
