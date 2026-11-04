//! [`RecallQuery`]: what a recall looks for and how wide each tier may
//! reach, built from `retrieval.yaml` once per new turn.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::FactKindId;

use super::declarations::QuorumUnit;

/// What a recall looks for and how wide each tier may reach.
///
/// The hints from the last user turn, the tier row limit (`subject_limit` for
/// the subject and linked tiers, 0 meaning `limit`), the link weight, depth
/// and per-hop decay, the
/// planner's synonyms, stop words and document-count ceiling, the asked
/// kinds that rank first in the context and subject tiers, and the
/// longest-alias rule.
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
    pub search_max_ndoc: u32,
    #[serde(default)]
    pub search_rare_alone_share: f64,
    #[serde(default)]
    pub subject_limit: u32,
    #[serde(default)]
    pub asked_kinds: Vec<FactKindId>,
    #[serde(default)]
    pub alias_longest_wins: bool,
    #[serde(default = "RecallQuery::default_links_depth")]
    pub links_depth: u32,
    #[serde(default = "RecallQuery::default_links_decay")]
    pub links_decay: f64,
    #[serde(default)]
    pub links_require_readable_anchor: bool,
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
            search_max_ndoc: 0,
            search_rare_alone_share: 0.0,
            subject_limit: 0,
            asked_kinds: Vec::new(),
            alias_longest_wins: false,
            links_depth: Self::default_links_depth(),
            links_decay: Self::default_links_decay(),
            links_require_readable_anchor: false,
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

    #[must_use]
    pub const fn default_links_depth() -> u32 {
        1
    }

    #[must_use]
    pub const fn default_links_decay() -> f64 {
        0.5
    }

    const fn default_search_max_df() -> f64 {
        0.2
    }

    const fn default_search_quorum() -> f64 {
        0.5
    }
}
