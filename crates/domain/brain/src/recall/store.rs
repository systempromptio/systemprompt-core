//! [`BrainRecallStore`]: the production [`FactRecallSource`] over the brain
//! tables. Each tier runs over the principal's working-set candidates on
//! the read pool and loads the facts in the tier's rank order; the recall
//! receipt is written as the `brain_recalls` row of truth with the
//! rendered block a same-turn request reuses. A receipt that carries a
//! late block records `skipped=deadline` together with that block.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use systemprompt_extension::brain::{
    FactRecallSource, RecallBlock, RecallError, RecallOutcome, RecallReceipt,
};
use systemprompt_identifiers::{BrainFactId, GatewayConversationId};
use systemprompt_manifest::profile::StalePolicy;
use systemprompt_models::brain::{
    BrainFact, FactRef, RecallAudit, RecallCtx, RecallQuery, StoredBlock, SynonymsDoc,
    WithheldReason,
};

use super::search_plan::SearchPolicy;
use super::tiers::{self, TierFilter};
use crate::models::NewRecall;
use crate::repository::{
    BrainRepositories, FactRepository, RecallRepository, SearchTerms, SearchTermsRepository,
};

const NO_LINKS: f64 = 0.0;

/// Reads recall tiers from, and records recalls into, the brain tables.
#[derive(Debug, Clone)]
pub struct BrainRecallStore {
    repos: BrainRepositories,
    stale: StalePolicy,
    working_set: bool,
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
            working_set: true,
            terms: Arc::new(Mutex::new(None)),
        }
    }

    async fn search_terms(&self) -> Result<Arc<SearchTerms>, RecallError> {
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

    #[must_use]
    pub const fn with_working_set(mut self, working_set: bool) -> Self {
        self.working_set = working_set;
        self
    }

    fn filter(&self, ctx: &RecallCtx, limit: u32, links_weight: f64) -> TierFilter {
        TierFilter {
            user: ctx.user_id.clone(),
            project: ctx.project.clone(),
            scopes: ctx.scopes.iter().map(|s| s.as_str().to_owned()).collect(),
            keys: ctx.held_keys.clone(),
            include_stale: self.stale == StalePolicy::Marker,
            as_of: ctx.as_of,
            limit: i64::from(limit),
            working_set: self.working_set,
            links_weight,
        }
    }

    async fn load_ranked(&self, ids: Vec<BrainFactId>) -> Result<Vec<BrainFact>, RecallError> {
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

fn refs(block: &RecallBlock) -> Vec<FactRef> {
    block
        .facts
        .iter()
        .map(|fact| FactRef {
            id: fact.id.clone(),
            revision: fact.revision,
        })
        .collect()
}

fn audit_of(receipt: &RecallReceipt, block: &RecallBlock) -> RecallAudit {
    RecallAudit {
        recall_id: receipt.recall_id.clone(),
        injected: refs(block),
        withheld: block.withheld.by_reason.clone(),
        tokens: block.tokens,
        as_of: block.as_of,
    }
}

#[must_use]
pub fn new_recall(receipt: &RecallReceipt) -> NewRecall {
    let mut conversation = receipt.conversation.clone();
    conversation.ai_request_id = Some(receipt.ai_request_id.clone());
    let (audit, rendered, skipped) = match (&receipt.outcome, &receipt.late) {
        (RecallOutcome::Injected(block), _) => {
            (audit_of(receipt, block), Some(block.render()), None)
        },
        (RecallOutcome::Skipped { reason }, Some(late)) => (
            audit_of(receipt, late),
            Some(late.render()),
            Some(reason.as_str()),
        ),
        (RecallOutcome::Skipped { reason }, None) => (
            RecallAudit {
                recall_id: receipt.recall_id.clone(),
                injected: Vec::new(),
                withheld: BTreeMap::new(),
                tokens: 0,
                as_of: receipt.recorded_at,
            },
            None,
            Some(reason.as_str()),
        ),
    };
    let mut builder = NewRecall::builder(
        receipt.principal.user_id.clone(),
        receipt.principal.client,
        audit,
    )
    .conversation(conversation);
    if let Some(rendered) = rendered {
        builder = builder.rendered_block(rendered);
    }
    if let Some(reason) = skipped {
        builder = builder.skipped(reason);
    }
    if let Some(project) = &receipt.principal.project {
        builder = builder.project(project.clone());
    }
    if let Some(workflow) = &receipt.workflow {
        builder = builder.workflow(workflow.clone());
    }
    builder.build()
}

#[async_trait::async_trait]
impl FactRecallSource for BrainRecallStore {
    async fn pinned(&self, ctx: &RecallCtx, limit: u32) -> Result<Vec<BrainFact>, RecallError> {
        let filter = self.filter(ctx, limit, NO_LINKS);
        let ids = tiers::pinned(self.repos.read_pool(), &filter).await?;
        self.load_ranked(ids).await
    }

    async fn by_subject(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError> {
        let hints = tiers::normalise_hints(&query.subject_hints);
        let filter = self.filter(ctx, query.limit, query.links_weight);
        let ids = tiers::by_subject(self.repos.read_pool(), &filter, &hints).await?;
        self.load_ranked(ids).await
    }

    async fn by_linked(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError> {
        let hints = tiers::normalise_hints(&query.subject_hints);
        let filter = self.filter(ctx, query.limit, query.links_weight);
        let ids = tiers::by_linked(self.repos.read_pool(), &filter, &hints).await?;
        self.load_ranked(ids).await
    }

    async fn by_context(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError> {
        let filter = self.filter(ctx, query.limit, query.links_weight);
        let ids = tiers::by_context(self.repos.read_pool(), &filter).await?;
        self.load_ranked(ids).await
    }

    async fn full_text(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError> {
        let filter = self.filter(ctx, query.limit, query.links_weight);
        let synonyms = SynonymsDoc {
            groups: query.synonyms.clone(),
        };
        let terms = self.search_terms().await?;
        let policy = SearchPolicy {
            max_df: query.search_max_df,
            quorum: query.search_quorum,
        };
        let input = tiers::FullTextInput {
            hints: &query.subject_hints,
            synonyms: &synonyms,
            terms: &terms,
            policy: &policy,
        };
        let ids = tiers::full_text(self.repos.read_pool(), &filter, input).await?;
        self.load_ranked(ids).await
    }

    async fn stored_block(
        &self,
        conversation: &GatewayConversationId,
    ) -> Result<Option<StoredBlock>, RecallError> {
        Ok(self.repos.recalls.stored_block_for(conversation).await?)
    }

    async fn record_recall(&self, receipt: &RecallReceipt) -> Result<(), RecallError> {
        RecallRepository::record(self.repos.write_pool(), &new_recall(receipt)).await?;
        Ok(())
    }

    async fn withheld_counts(
        &self,
        ctx: &RecallCtx,
    ) -> Result<BTreeMap<WithheldReason, u32>, RecallError> {
        let filter = self.filter(ctx, 0, NO_LINKS);
        Ok(tiers::withheld_counts(self.repos.read_pool(), &filter).await?)
    }
}
