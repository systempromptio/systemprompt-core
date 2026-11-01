//! [`BrainRecallStore`]: the production [`FactRecallSource`] over the brain
//! tables. Each tier runs its scope-filtered query on the read pool and
//! loads the facts in the tier's rank order; the recall receipt is written
//! as the `brain_recalls` row of truth with the rendered block a same-turn
//! request reuses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_extension::brain::{FactRecallSource, RecallError, RecallOutcome, RecallReceipt};
use systemprompt_identifiers::{BrainFactId, GatewayConversationId};
use systemprompt_manifest::profile::StalePolicy;
use systemprompt_models::brain::{
    BrainFact, FactRef, RecallAudit, RecallCtx, RecallQuery, StoredBlock,
};

use super::tiers::{self, TierFilter};
use crate::models::NewRecall;
use crate::repository::{BrainRepositories, FactRepository, RecallRepository};

/// Reads recall tiers from, and records recalls into, the brain tables.
#[derive(Debug, Clone)]
pub struct BrainRecallStore {
    repos: BrainRepositories,
    stale: StalePolicy,
}

impl BrainRecallStore {
    #[must_use]
    pub const fn new(repos: BrainRepositories, stale: StalePolicy) -> Self {
        Self { repos, stale }
    }

    fn filter(&self, ctx: &RecallCtx, limit: u32) -> TierFilter {
        TierFilter {
            user: ctx.user_id.clone(),
            project: ctx.project.clone(),
            scopes: ctx.scopes.iter().map(|s| s.as_str().to_owned()).collect(),
            include_stale: self.stale == StalePolicy::Marker,
            now: ctx.now,
            limit: i64::from(limit),
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

#[must_use]
pub fn new_recall(receipt: &RecallReceipt) -> NewRecall {
    let mut conversation = receipt.conversation.clone();
    conversation.ai_request_id = Some(receipt.ai_request_id.clone());
    let (audit, rendered, skipped) = match &receipt.outcome {
        RecallOutcome::Injected(block) => (
            RecallAudit {
                recall_id: receipt.recall_id.clone(),
                injected: block
                    .facts
                    .iter()
                    .map(|fact| FactRef {
                        id: fact.id.clone(),
                        revision: fact.revision,
                    })
                    .collect(),
                withheld: block.withheld.by_reason.clone(),
                tokens: block.tokens,
                as_of: block.as_of,
            },
            Some(block.render()),
            None,
        ),
        RecallOutcome::Skipped { reason } => (
            RecallAudit {
                recall_id: receipt.recall_id.clone(),
                injected: Vec::new(),
                withheld: std::collections::BTreeMap::new(),
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
        let ids = tiers::pinned(self.repos.read_pool(), &self.filter(ctx, limit)).await?;
        self.load_ranked(ids).await
    }

    async fn by_subject(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError> {
        let hints = tiers::normalise_hints(&query.subject_hints);
        let filter = self.filter(ctx, query.limit);
        let ids = tiers::by_subject(self.repos.read_pool(), &filter, &hints).await?;
        self.load_ranked(ids).await
    }

    async fn by_context(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError> {
        let filter = self.filter(ctx, query.limit);
        let ids = tiers::by_context(self.repos.read_pool(), &filter).await?;
        self.load_ranked(ids).await
    }

    async fn full_text(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError> {
        let filter = self.filter(ctx, query.limit);
        let ids = tiers::full_text(self.repos.read_pool(), &filter, &query.subject_hints).await?;
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
}
