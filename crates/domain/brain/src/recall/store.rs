//! [`BrainRecallStore`]: the production [`FactRecallSource`] over the brain
//! tables. The tiers run over the principal's working-set candidates on
//! the read pool and return ids (`store_tiers`); the fused facts are loaded
//! once through the [`FactLoader`] this store also is. The recall receipt
//! is written as the `brain_recalls` row of truth with the rendered block
//! a same-turn request reuses (`store_receipt`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use systemprompt_extension::brain::RecallError;
use systemprompt_identifiers::BrainFactId;
use systemprompt_manifest::profile::StalePolicy;
use systemprompt_models::brain::{BrainFact, RecallCtx, RecallQuery};

use super::introduce::FactLoader;
use super::tiers::TierFilter;
use crate::repository::{BrainRepositories, FactRepository, SearchTerms, SearchTermsRepository};

/// Reads recall tiers from, and records recalls into, the brain tables.
#[derive(Debug, Clone)]
pub struct BrainRecallStore {
    pub(super) repos: BrainRepositories,
    stale: StalePolicy,
    terms: TermsCache,
}

type TermsCache = Arc<Mutex<Option<(Instant, Arc<SearchTerms>)>>>;

const TERMS_TTL: Duration = Duration::from_secs(60);

impl BrainRecallStore {
    #[must_use]
    pub fn new(repos: BrainRepositories, stale: StalePolicy) -> Self {
        Self {
            repos,
            stale,
            terms: Arc::new(Mutex::new(None)),
        }
    }

    pub(super) async fn search_terms(&self) -> Result<Arc<SearchTerms>, RecallError> {
        if let Some(terms) = self.cached_terms() {
            return Ok(terms);
        }
        let terms = Arc::new(SearchTermsRepository::load(self.repos.read_pool()).await?);
        if let Ok(mut slot) = self.terms.lock() {
            *slot = Some((Instant::now(), Arc::clone(&terms)));
        }
        Ok(terms)
    }

    fn cached_terms(&self) -> Option<Arc<SearchTerms>> {
        let slot = self.terms.lock().ok()?;
        let fresh = slot
            .as_ref()
            .filter(|(at, _)| at.elapsed() < TERMS_TTL)
            .map(|(_, terms)| Arc::clone(terms));
        drop(slot);
        fresh
    }

    pub(super) fn filter(&self, ctx: &RecallCtx, limit: u32, links_weight: f64) -> TierFilter {
        TierFilter {
            user: ctx.user_id.clone(),
            project: ctx.project.clone(),
            scopes: ctx.scopes.iter().map(|s| s.as_str().to_owned()).collect(),
            keys: ctx.held_keys.clone(),
            include_stale: self.stale == StalePolicy::Marker,
            as_of: ctx.as_of,
            limit: i64::from(limit),
            links_weight,
            alias_min_sim: RecallQuery::default_alias_min_sim(),
            max_subjects: i64::from(RecallQuery::default_max_subjects()),
        }
    }

    pub(super) async fn load_ranked(
        &self,
        ids: Vec<BrainFactId>,
    ) -> Result<Vec<BrainFact>, RecallError> {
        let rank: HashMap<BrainFactId, usize> = ids
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();
        let mut facts = FactRepository::load(self.repos.read_pool(), &ids).await?;
        facts.sort_by_key(|fact| rank.get(&fact.id).copied().unwrap_or(usize::MAX));
        Ok(facts)
    }
}

#[async_trait::async_trait]
impl FactLoader for BrainRecallStore {
    async fn load(&self, ids: &[BrainFactId]) -> Result<Vec<BrainFact>, RecallError> {
        Ok(FactRepository::load(self.repos.read_pool(), ids).await?)
    }
}
