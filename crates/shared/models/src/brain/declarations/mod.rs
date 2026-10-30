//! Declarative brain config: the YAML shapes under `services/brain/` and
//! their cross-reference validation.
//!
//! `topics.yaml`, `filters.yaml`, `importance.yaml` and `retrieval.yaml` are
//! single documents; `kinds/`, `interests/` and `profiles/` hold one
//! declaration per file. [`BrainDocs`] is the whole set and [`validate`]
//! checks it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod importance;
mod validate;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{FactKindId, InterestId, TopicId, UserId, WorkflowId};

pub use importance::{
    BaseImportance, ImportanceConfig, InContextImportance, ModelHint, Recency, RetrievalDoc,
    TierWeights, UsageWeights,
};
pub use validate::{BrainDeclError, validate};

use super::envelope::Pathway;
use super::kind_decl::KindDecl;
use crate::notifications::AudienceSubject;

/// How finely a topic or interest may report about people.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Granularity {
    #[default]
    Aggregate,
    Individual,
}

/// A controlled topic and the audience it widens fact scope to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopicDecl {
    pub id: TopicId,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub audience: Vec<AudienceSubject>,
    #[serde(default)]
    pub granularity: Granularity,
}

/// `topics.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TopicsDoc {
    pub topics: Vec<TopicDecl>,
}

/// How an interest's matches are delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterestDelivery {
    Memory,
    Digest,
    Alert,
}

/// Deterministic predicates of an interest; an empty list matches all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct InterestMatch {
    pub kinds: Vec<FactKindId>,
    pub topics: Vec<TopicId>,
    pub subjects: Vec<String>,
    pub keywords: Vec<String>,
}

/// A standing question with an owner, pathways and delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterestDecl {
    pub id: InterestId,
    pub name: String,
    pub owner: UserId,
    pub question: String,
    #[serde(default)]
    pub pathways: Vec<Pathway>,
    #[serde(default)]
    pub r#match: InterestMatch,
    #[serde(default)]
    pub granularity: Granularity,
    #[serde(default)]
    pub audience: Vec<AudienceSubject>,
    #[serde(default)]
    pub delivery: Vec<InterestDelivery>,
    #[serde(default)]
    pub expires: Option<DateTime<Utc>>,
}

/// Which actors an event filter admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ActorFilter {
    pub humans: bool,
    pub attended_agents: bool,
    pub unattended_agents: bool,
}

impl Default for ActorFilter {
    fn default() -> Self {
        Self {
            humans: true,
            attended_agents: true,
            unattended_agents: false,
        }
    }
}

/// One declared event filter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterDecl {
    pub name: String,
    pub pathways: Vec<Pathway>,
    #[serde(default)]
    pub actors: ActorFilter,
    #[serde(default)]
    pub kinds: Vec<FactKindId>,
    #[serde(default)]
    pub debounce_secs: Option<u32>,
}

/// `filters.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FiltersDoc {
    pub filters: Vec<FilterDecl>,
}

/// A workflow profile under `profiles/*.yaml`: the kinds a workflow cares
/// about, who may confirm in it, and its injection limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowProfileDecl {
    pub id: WorkflowId,
    pub name: String,
    #[serde(default)]
    pub kinds: Vec<FactKindId>,
    #[serde(default)]
    pub confirm_role: Option<String>,
    #[serde(default)]
    pub quotas: BTreeMap<FactKindId, u32>,
    #[serde(default)]
    pub budget_tokens: Option<u32>,
}

/// Every brain declaration, loaded together.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BrainDocs {
    pub topics: TopicsDoc,
    pub filters: FiltersDoc,
    pub importance: ImportanceConfig,
    pub retrieval: RetrievalDoc,
    pub kinds: Vec<KindDecl>,
    pub interests: Vec<InterestDecl>,
    pub workflows: Vec<WorkflowProfileDecl>,
}
