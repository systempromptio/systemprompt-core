//! `importance.yaml` and `retrieval.yaml`: the scoring formula's weights
//! and the retrieval tiers' fusion weights and quotas.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{FactKindId, WorkflowId};

use crate::brain::source::SourceClass;

/// The importance formula: a stored base and an in-context multiplier.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ImportanceConfig {
    pub base: BaseImportance,
    pub in_context: InContextImportance,
}

/// Importance stored on write.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BaseImportance {
    pub kind_weight: BTreeMap<FactKindId, f64>,
    pub source_weight: BTreeMap<SourceClass, f64>,
    pub model_hint: Option<ModelHint>,
    pub confirmed_by_owner: f64,
}

/// A field whose value the extractor scores, and that score's weight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelHint {
    pub field: String,
    pub weight: f64,
}

/// Importance computed per request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct InContextImportance {
    pub workflow_weight: BTreeMap<WorkflowId, BTreeMap<FactKindId, f64>>,
    pub subject_match: f64,
    pub recency: Recency,
    pub usage: UsageWeights,
}

/// Exponential recency decay.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Recency {
    pub half_life_days: u32,
}

impl Default for Recency {
    fn default() -> Self {
        Self { half_life_days: 90 }
    }
}

/// How past citation and neglect move a fact's score. The rates are the
/// `brain_usage` job's Laplace-smoothed shares, so the defaults add at most
/// `cited_rate_weight` and subtract at most `ignored_penalty`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct UsageWeights {
    pub cited_rate_weight: f64,
    pub ignored_penalty: f64,
}

impl Default for UsageWeights {
    fn default() -> Self {
        Self {
            cited_rate_weight: 0.2,
            ignored_penalty: 0.1,
        }
    }
}

/// `retrieval.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RetrievalDoc {
    pub tiers: TierWeights,
    pub rrf_k: u32,
    pub max_facts: u32,
    pub quotas: BTreeMap<FactKindId, u32>,
    pub links_weight: f64,
    pub search_max_df: f64,
    pub search_quorum: f64,
}

impl Default for RetrievalDoc {
    fn default() -> Self {
        Self {
            tiers: TierWeights::default(),
            rrf_k: 60,
            max_facts: 12,
            quotas: BTreeMap::new(),
            links_weight: 0.6,
            search_max_df: 0.2,
            search_quorum: 0.5,
        }
    }
}

/// Reciprocal-rank-fusion weight of each retrieval tier.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TierWeights {
    pub pinned: f64,
    pub subject: f64,
    pub linked: f64,
    pub context: f64,
    pub full_text: f64,
    pub extension: f64,
}

impl Default for TierWeights {
    fn default() -> Self {
        Self {
            pinned: 1.0,
            subject: 1.0,
            linked: 0.6,
            context: 0.8,
            full_text: 0.6,
            extension: 0.0,
        }
    }
}
