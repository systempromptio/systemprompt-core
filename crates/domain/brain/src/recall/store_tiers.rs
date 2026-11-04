//! The [`FactRecallSource`] of [`BrainRecallStore`]: the five tiers as ids
//! in one concurrent read (`tier_ids`), each tier as loaded facts for a
//! caller that reads one, the lifecycle count, the stored block and the
//! receipt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_extension::brain::{FactRecallSource, RecallError, RecallReceipt, TierIds};
use systemprompt_identifiers::GatewayConversationId;
use systemprompt_models::brain::{
    BrainFact, RecallCtx, RecallQuery, StoredBlock, SynonymsDoc, WithheldReason,
};

use super::search_plan::SearchPolicy;
use super::store::BrainRecallStore;
use super::store_receipt::new_recall;
use super::tiers::{self, matched};
use crate::repository::RecallRepository;

const NO_LINKS: f64 = 0.0;

const fn policy_of(query: &RecallQuery) -> SearchPolicy {
    SearchPolicy {
        max_df: query.search_max_df,
        quorum: query.search_quorum,
        retry_quorum: query.search_retry_quorum,
        quorum_over: query.search_quorum_over,
    }
}

impl BrainRecallStore {
    async fn matched_for(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<(tiers::TierFilter, matched::MatchedSubjects), RecallError> {
        let hints = tiers::normalise_hints(&query.subject_hints);
        let filter = self
            .filter(ctx, query.limit, query.links_weight)
            .with_subjects(query);
        let matched = matched::resolve(self.repos.read_pool(), &filter, &hints).await?;
        Ok((filter, matched))
    }
}

#[async_trait::async_trait]
impl FactRecallSource for BrainRecallStore {
    async fn tier_ids(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
        pinned_limit: u32,
    ) -> Result<Vec<TierIds>, RecallError> {
        let hints = tiers::normalise_hints(&query.subject_hints);
        let filter = self
            .filter(ctx, query.limit, query.links_weight)
            .with_subjects(query);
        let synonyms = SynonymsDoc {
            groups: query.synonyms.clone(),
        };
        let terms = self.search_terms().await?;
        let policy = policy_of(query);
        let input = tiers::RankedInput {
            filter: &filter,
            pinned_limit: i64::from(pinned_limit),
            hints: &hints,
            full_text: tiers::FullTextInput {
                hints: &query.subject_hints,
                synonyms: &synonyms,
                terms: &terms,
                policy: &policy,
            },
        };
        Ok(tiers::ranked(self.repos.read_pool(), input).await?)
    }

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
        let (filter, matched) = self.matched_for(query, ctx).await?;
        let ids = tiers::by_subject(self.repos.read_pool(), &filter, &matched).await?;
        self.load_ranked(ids).await
    }

    async fn by_linked(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError> {
        let (filter, matched) = self.matched_for(query, ctx).await?;
        let ids = tiers::by_linked(self.repos.read_pool(), &filter, &matched).await?;
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
        let policy = policy_of(query);
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
