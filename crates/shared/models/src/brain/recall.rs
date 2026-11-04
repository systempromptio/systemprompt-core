//! Read-side values: the recall query and context, scored facts, the
//! injection plan and the audit of what was injected or withheld.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{BrainFactId, BrainRecallId, UserId, WorkflowId};

use super::declarations::QuorumUnit;
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
    Linked,
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
    Disputed,
    Quota,
    MaxFacts,
    Budget,
    LowScore,
    ContextCap,
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
            Self::Disputed => "disputed",
            Self::Quota => "quota",
            Self::MaxFacts => "max_facts",
            Self::Budget => "budget",
            Self::LowScore => "low_score",
            Self::ContextCap => "context_cap",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Who is asking, from where, at what time.
///
/// `now` is computed once per recall and is the only clock the tiers,
/// the lifecycle check and the importance score read. `as_of` is the
/// instant the facts are read at: `now` unless the request asked for an
/// earlier view, in which case every tier admits only facts observed by
/// then and valid then. `held_keys` are the working-set scope keys the
/// principal holds (`org`, `user:<id>`, `project:<key>` and every
/// audience subject a resolver enumerated), built once per recall.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallCtx {
    pub user_id: UserId,
    pub actor: RequestActor,
    pub client: ClientKind,
    pub project: Option<String>,
    pub workflow: Option<WorkflowId>,
    pub scopes: Vec<ScopeLevel>,
    pub now: DateTime<Utc>,
    pub as_of: DateTime<Utc>,
    #[serde(default)]
    pub held_keys: Vec<String>,
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
            as_of: now,
            held_keys: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_as_of(mut self, as_of: DateTime<Utc>) -> Self {
        self.as_of = as_of.min(self.now);
        self
    }

    #[must_use]
    pub fn with_held_keys(mut self, keys: Vec<String>) -> Self {
        self.held_keys = keys;
        self
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

/// What a recall looks for and how wide each tier may reach.
///
/// Subject hints come from the last user turn; each tier has its own row
/// limit (`subject_limit` is the subject and linked tiers' own, 0 meaning
/// `limit`); a one-hop link match earns `links_weight` relative to a
/// direct subject match; the synonym groups expand each full-text query
/// word and the stop words are dropped before it is planned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallQuery {
    pub subject_hints: String,
    pub limit: u32,
    #[serde(default = "RecallQuery::default_links_weight")]
    pub links_weight: f64,
    #[serde(default)]
    pub synonyms: Vec<Vec<String>>,
    #[serde(default = "RecallQuery::default_search_max_df")]
    pub search_max_df: f64,
    #[serde(default = "RecallQuery::default_search_quorum")]
    pub search_quorum: f64,
    #[serde(default)]
    pub search_retry_quorum: f64,
    #[serde(default)]
    pub search_quorum_over: QuorumUnit,
    #[serde(default = "RecallQuery::default_alias_min_sim")]
    pub alias_min_sim: f64,
    #[serde(default = "RecallQuery::default_max_subjects")]
    pub max_subjects: u32,
    #[serde(default)]
    pub search_stop_words: Vec<String>,
    #[serde(default)]
    pub subject_limit: u32,
}

impl RecallQuery {
    #[must_use]
    pub const fn new(subject_hints: String, limit: u32) -> Self {
        Self {
            subject_hints,
            limit,
            links_weight: Self::default_links_weight(),
            synonyms: Vec::new(),
            search_max_df: Self::default_search_max_df(),
            search_quorum: Self::default_search_quorum(),
            search_retry_quorum: 0.0,
            search_quorum_over: QuorumUnit::Groups,
            alias_min_sim: Self::default_alias_min_sim(),
            max_subjects: Self::default_max_subjects(),
            search_stop_words: Vec::new(),
            subject_limit: 0,
        }
    }

    #[must_use]
    pub const fn subject_tier_limit(&self) -> u32 {
        if self.subject_limit == 0 {
            self.limit
        } else {
            self.subject_limit
        }
    }

    #[must_use]
    pub const fn default_alias_min_sim() -> f64 {
        0.6
    }

    #[must_use]
    pub const fn default_max_subjects() -> u32 {
        4
    }

    const fn default_links_weight() -> f64 {
        0.6
    }

    const fn default_search_max_df() -> f64 {
        0.2
    }

    const fn default_search_quorum() -> f64 {
        0.5
    }
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

/// The block a conversation's last block-producing recall stored.
///
/// It names the facts at their revisions; same-turn reuse resends it
/// unchanged. `late` marks a block a recall produced after its gateway
/// deadline: it was stored, never injected, for the turn's next request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredBlock {
    pub rendered: String,
    pub as_of: DateTime<Utc>,
    pub facts: Vec<FactRef>,
    #[serde(default)]
    pub late: bool,
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
