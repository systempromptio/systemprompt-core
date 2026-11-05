//! The `retrieval.yaml` checks: every tier and knob weight inside the unit
//! interval, no zero limit, `subject_limit` and `links_depth` under their
//! caps, every stop word a lower-case token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::RetrievalDoc;
use super::validate::{BrainDeclError, MAX_LINKS_DEPTH, MAX_SUBJECT_LIMIT, weight};

pub(super) fn validate_retrieval(doc: &RetrievalDoc) -> Result<(), BrainDeclError> {
    let tiers = &doc.tiers;
    weight("tiers.pinned", tiers.pinned)?;
    weight("tiers.subject", tiers.subject)?;
    weight("tiers.context", tiers.context)?;
    weight("tiers.full_text", tiers.full_text)?;
    weight("tiers.extension", tiers.extension)?;
    weight("tiers.linked", tiers.linked)?;
    weight("links_weight", doc.links_weight)?;
    weight("search_max_df", doc.search_max_df)?;
    weight("search_quorum", doc.search_quorum)?;
    weight("search_retry_quorum", doc.search_retry_quorum)?;
    weight("alias_min_sim", doc.alias_min_sim)?;
    weight("linked_share", doc.linked_share)?;
    weight("context_share", doc.context_share)?;
    weight("usage_weight", doc.usage_weight)?;
    weight("links_decay", doc.links_decay)?;
    weight("search_rare_alone_share", doc.search_rare_alone_share)?;
    if !doc.min_fused_score.is_finite() || doc.min_fused_score < 0.0 {
        return Err(BrainDeclError::NegativeScore {
            name: "min_fused_score",
            value: doc.min_fused_score,
        });
    }
    for (name, value) in [
        ("max_facts", doc.max_facts),
        ("max_subjects", doc.max_subjects),
        ("extension_max_candidates", doc.extension_max_candidates),
        ("links_depth", doc.links_depth),
        ("reranker_timeout_ms", doc.reranker_timeout_ms),
    ] {
        if value == 0 {
            return Err(BrainDeclError::ZeroLimit { name });
        }
    }
    if doc.subject_limit > MAX_SUBJECT_LIMIT {
        return Err(BrainDeclError::LimitOverMax {
            name: "subject_limit",
            value: doc.subject_limit,
            max: MAX_SUBJECT_LIMIT,
        });
    }
    if doc.links_depth > MAX_LINKS_DEPTH {
        return Err(BrainDeclError::LimitOverMax {
            name: "links_depth",
            value: doc.links_depth,
            max: MAX_LINKS_DEPTH,
        });
    }
    if let Some(word) = doc.search_stop_words.iter().find(|w| !is_token(w)) {
        return Err(BrainDeclError::StopWordNotToken { word: word.clone() });
    }
    Ok(())
}

pub(super) fn is_token(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_alphanumeric() && !c.is_uppercase())
}
