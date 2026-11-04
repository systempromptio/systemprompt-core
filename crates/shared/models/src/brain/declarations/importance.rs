//! `importance.yaml` and `retrieval.yaml`: the scoring formula's weights
//! and the retrieval tiers' fusion weights, quotas and the fill policy.
//!
//! The fill knobs: `fill_order` (`rank`: fused rank
//! first, importance second; `importance`: the previous order);
//! `linked_share` of `max_facts` reserved for one-hop link hits when a
//! subject is named (0 reserves nothing); `min_fused_score` below which a
//! fact is withheld as `low_score` (0 withholds nothing); `context_share`
//! of `max_facts` that facts found only by the context tier may take when
//! a subject is named (1 caps nothing, 0 admits them only when no subject
//! is named). The planner knobs: `search_retry_quorum` runs the full-text
//! tier again at that quorum when the first pass returns nothing (0 runs
//! no second pass); `search_quorum_over` counts the quorum over synonym
//! groups or over the query words they absorbed. The subject-tier knobs:
//! `alias_min_sim` is the least alias similarity that names a subject
//! (the trigram operator's own threshold, 0.6, is its floor) and
//! `max_subjects` the most subjects one message may name. The extension
//! knobs: `extension_max_candidates` is the most facts the
//! extension retrievers may introduce beyond what the core tiers found,
//! and `reranker_timeout_ms` (default 150, never 0) is how long the
//! registered reranker may take to reorder the visible facts before the
//! fused order stands.
//!
//! Three knobs gate what a message with no lexical signal can pull in.
//! `context_requires_signal` withholds every fact the context tier alone
//! found (as `context_cap`) when the recall names no subject and the
//! full-text tier returned nothing, so a message that carries no word of
//! any fact injects none of the attributed project's facts (false keeps
//! them). `search_stop_words` are lower-case single tokens the planner
//! drops before grouping and quorum, so a scaffold word is never the one
//! a fact must carry; once any are declared a message with no informative
//! group left plans no query at all instead of the disjunction of its
//! common words (empty keeps that disjunction). `subject_limit` is the
//! row limit of the subject and linked tiers (0 keeps the shared tier
//! limit, `max_facts × 2` clamped to 8..=50; at most 100).
//!
//! The depth knobs: `links_depth` (1 or 2, default 1) is how many
//! subject hops the linked tier walks from a named subject. At 1 it
//! reaches the facts one `brain_fact_links` hop away, as before. At 2 it
//! also reaches, through the bridges a fact declares between its subject
//! and the subjects its entity fields name (`brain_subject_links`), the
//! facts about a bridged subject and the facts linked to one, each scored
//! `links_decay` (0..=1, default 0.5) lower per hop: a fact about a bridged
//! subject scores `links_weight × links_decay`, a fact linked to one
//! `links_weight × links_decay × links_weight`. Every hit stays in the
//! linked tier.
//!
//! Two more name rules. `alias_longest_wins` drops a subject matched at
//! `1.0` whose matched alias is a proper substring of another matched
//! subject's exact alias (`larchmont` inside `larchmont platform`), so a
//! client is not named by its project's id. `unknown_name_withholds_full_text`
//! withholds the full-text tier's hits as `unknown_name` when no subject
//! matched and the message carries a capitalised, non-sentence-initial
//! word of three or more letters that no fact carries: the message is
//! about an entity the brain does not know, and a shared-vocabulary hit
//! is not an answer about it. A message that asks a declared kind
//! (`KindDecl::asks`) counts as a signal for `context_requires_signal`,
//! and facts of the asked kinds rank first in the context and subject
//! tiers.
//!
//! The usage knobs close the citation loop on the fill:
//! `usage_weight` adds `usage_weight × (cited_rate − ignored_rate)` to a
//! fact's fused score before `min_fused_score` and the fill order (0 adds
//! nothing). The rates are the `brain_usage` job's Laplace shares, which
//! sum to one, so the term is centred: a fact never injected sits at
//! 0.5/0.5 and a fact the job has not yet rated at 0/0, and both add
//! exactly zero. `usage_min_recalls` is the fewest injections before the
//! job rates a fact at all; under it the fact keeps the prior, so the
//! term stays zero and a new fact is not punished (0 rates every fact).
//! The in-context importance term is weighted separately by
//! [`UsageWeights`] in `importance.yaml`.
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
    pub search_retry_quorum: f64,
    pub search_quorum_over: QuorumUnit,
    pub alias_min_sim: f64,
    pub max_subjects: u32,
    pub fill_order: FillOrder,
    pub linked_share: f64,
    pub min_fused_score: f64,
    pub context_share: f64,
    pub usage_weight: f64,
    pub usage_min_recalls: u32,
    pub extension_max_candidates: u32,
    pub context_requires_signal: bool,
    pub search_stop_words: Vec<String>,
    pub search_max_ndoc: u32,
    pub subject_limit: u32,
    pub alias_longest_wins: bool,
    pub unknown_name_withholds_full_text: bool,
    pub links_depth: u32,
    pub links_decay: f64,
    pub reranker_timeout_ms: u32,
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
            search_retry_quorum: 0.0,
            search_quorum_over: QuorumUnit::default(),
            alias_min_sim: 0.6,
            max_subjects: 4,
            fill_order: FillOrder::default(),
            linked_share: 0.0,
            min_fused_score: 0.0,
            context_share: 1.0,
            usage_weight: 0.0,
            usage_min_recalls: 0,
            extension_max_candidates: 24,
            context_requires_signal: false,
            search_stop_words: Vec::new(),
            search_max_ndoc: 0,
            subject_limit: 0,
            alias_longest_wins: false,
            unknown_name_withholds_full_text: false,
            links_depth: 1,
            links_decay: 0.5,
            reranker_timeout_ms: 150,
        }
    }
}

/// What the full-text planner counts its quorum over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuorumUnit {
    #[default]
    Groups,
    Words,
}

/// Which key orders the budget fill.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillOrder {
    #[default]
    Rank,
    Importance,
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
