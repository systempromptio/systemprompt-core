//! Extension retrievers as a retrieval tier.
//!
//! Each [`FactRetriever`] returns fact ids in its rank order. An id a
//! core tier already returned is fused with the extension's rank; an id
//! none returned is *introduced*: loaded through the [`FactLoader`],
//! admitted by the tier SQL's rule ([`super::admit`]) and fused as
//! [`RetrievalTier::Extension`] like any other hit, so the lifecycle and
//! visibility checks run over it exactly as over a core hit. Introduced
//! ids are taken round-robin by rank across the retrievers, deduplicated,
//! and capped at `extension_max_candidates`. A retriever's list is ranked
//! over the ids that are known (a core hit or an admitted introduction),
//! so an id that was not loaded or not admitted takes no rank. A zero
//! `tiers.extension` weight disables introduction and contributes nothing
//! to the fusion, so the recall is the one the core tiers alone produce.
//! A retriever or loader that fails is logged and contributes nothing;
//! the recall goes on over what the core tiers found. [`load_tiers`] is
//! the one load of every id the core tiers returned, dropping from each
//! tier an id the load did not return.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use systemprompt_extension::brain::{FactRetriever, RecallError, TierIds};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{BrainFact, RetrievalTier};

/// Loads facts by id for the recall; held as `Arc<dyn FactLoader>`.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait FactLoader: Send + Sync {
    async fn load(&self, ids: &[BrainFactId]) -> Result<Vec<BrainFact>, RecallError>;
}

#[must_use]
pub fn known(tiers: &[TierIds]) -> HashSet<BrainFactId> {
    tiers
        .iter()
        .flat_map(|hits| hits.ids.iter())
        .cloned()
        .collect()
}

#[must_use]
pub fn unseen<S: BuildHasher>(
    known: &HashSet<BrainFactId, S>,
    ranked: &[Vec<BrainFactId>],
    cap: u32,
) -> Vec<BrainFactId> {
    let cap = usize::try_from(cap).unwrap_or(usize::MAX);
    let deepest = ranked.iter().map(Vec::len).max().unwrap_or(0);
    let mut out: Vec<BrainFactId> = Vec::new();
    for rank in 0..deepest {
        for ids in ranked {
            let Some(id) = ids.get(rank) else {
                continue;
            };
            if out.len() >= cap {
                return out;
            }
            if !known.contains(id) && !out.contains(id) {
                out.push(id.clone());
            }
        }
    }
    out
}

#[must_use]
pub fn extension_hits<S: BuildHasher>(
    ranked: &[Vec<BrainFactId>],
    known: &HashSet<BrainFactId, S>,
) -> Vec<TierIds> {
    ranked
        .iter()
        .map(|ids| {
            let ids = ids
                .iter()
                .filter(|id| known.contains(*id))
                .cloned()
                .collect();
            TierIds::new(RetrievalTier::Extension, ids)
        })
        .collect()
}

pub async fn load_tiers(
    loader: &dyn FactLoader,
    tiers: &mut [TierIds],
) -> Result<HashMap<BrainFactId, BrainFact>, RecallError> {
    let ids: Vec<BrainFactId> = known(tiers).into_iter().collect();
    let facts: HashMap<BrainFactId, BrainFact> = loader
        .load(&ids)
        .await?
        .into_iter()
        .map(|fact| (fact.id.clone(), fact))
        .collect();
    for hits in tiers.iter_mut() {
        hits.ids.retain(|id| facts.contains_key(id));
    }
    Ok(facts)
}

pub async fn loaded(loader: &dyn FactLoader, ids: &[BrainFactId]) -> Vec<BrainFact> {
    match loader.load(ids).await {
        Ok(facts) => facts,
        Err(error) => {
            tracing::warn!(
                requested = ids.len(),
                error = %error,
                "Brain fact loader failed; the introduced candidates are skipped"
            );
            Vec::new()
        },
    }
}

pub fn ranked_of(
    retrievers: &[std::sync::Arc<dyn FactRetriever>],
    results: Vec<Result<Vec<BrainFactId>, RecallError>>,
) -> Vec<Vec<BrainFactId>> {
    retrievers
        .iter()
        .zip(results)
        .filter_map(|(retriever, result)| match result {
            Ok(ids) => Some(ids),
            Err(error) => {
                tracing::warn!(
                    retriever = retriever.name(),
                    error = %error,
                    "Brain retriever failed; its ranking is skipped"
                );
                None
            },
        })
        .collect()
}
