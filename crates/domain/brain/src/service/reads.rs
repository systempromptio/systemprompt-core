//! Reads for the REST and MCP surfaces: one fact, a filtered listing, a
//! fact's revision timeline, and the why-view (the in-context importance
//! terms and whether recall would withhold it). Each applies recall's
//! per-fact visibility; a hidden fact reads as not found.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_identifiers::BrainFactId;
use systemprompt_manifest::profile::StalePolicy;
use systemprompt_models::brain::{BrainFact, FactRevision, RecallCtx, WithheldReason};

use super::{BrainCaller, BrainService};
use crate::error::{BrainError, BrainResult};
use crate::models::FactFilter;
use crate::recall::score::{self, ImportanceTerms, ScoreInput};
use crate::recall::visible::lifecycle_withheld;

/// Why a fact would rank where it does for a recall context.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FactWhy {
    pub fact: BrainFact,
    pub terms: ImportanceTerms,
    pub withheld: Option<WithheldReason>,
}

impl BrainService {
    pub async fn get(&self, caller: &BrainCaller, id: &BrainFactId) -> BrainResult<BrainFact> {
        let fact = self
            .repos
            .facts
            .find(id)
            .await?
            .ok_or_else(|| BrainError::NotFound(id.clone()))?;
        self.require_visible(caller, fact).await
    }

    pub async fn search(
        &self,
        caller: &BrainCaller,
        filter: &FactFilter,
    ) -> BrainResult<Vec<BrainFact>> {
        let facts = self.repos.facts.list(filter).await?;
        self.only_visible(caller, facts).await
    }

    pub async fn timeline(
        &self,
        caller: &BrainCaller,
        id: &BrainFactId,
    ) -> BrainResult<Vec<FactRevision>> {
        self.get(caller, id).await?;
        Ok(self.repos.revisions.list(id).await?)
    }

    pub async fn why(
        &self,
        caller: &BrainCaller,
        id: &BrainFactId,
        ctx: &RecallCtx,
    ) -> BrainResult<FactWhy> {
        let fact = self.get(caller, id).await?;
        let declared = self.declared.load().await?;
        let input = ScoreInput {
            config: &declared.importance,
            ctx,
            subject_matched: false,
        };
        let terms = score::in_context(&fact, &input);
        let inject_stale = self.profile.recall.stale == StalePolicy::Marker;
        let withheld = lifecycle_withheld(&fact, ctx.now, inject_stale);
        Ok(FactWhy {
            fact,
            terms,
            withheld,
        })
    }
}
