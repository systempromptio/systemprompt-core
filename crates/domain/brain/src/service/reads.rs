//! Reads for the REST and MCP surfaces: one fact, a filtered listing, a
//! fact's revision timeline, and the why-view (the in-context importance
//! terms and whether recall would withhold it). Each applies recall's
//! per-fact visibility; a hidden fact reads as not found. A listing reads
//! only facts reachable through the caller's working-set keys, in keyset
//! pages, and keeps reading until the page is full of visible facts or
//! the listing ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_identifiers::BrainFactId;
use systemprompt_manifest::profile::StalePolicy;
use systemprompt_models::brain::{BrainFact, FactRevision, RecallCtx, WithheldReason};

use super::{BrainCaller, BrainService};
use crate::error::{BrainError, BrainResult};
use crate::models::{FactCursor, FactFilter, FactPage};
use crate::recall::held;
use crate::recall::lifecycle::lifecycle_withheld;
use crate::recall::score::{self, ImportanceTerms, ScoreInput};

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

    pub async fn search(&self, caller: &BrainCaller, filter: &FactFilter) -> BrainResult<FactPage> {
        let want = usize::try_from(filter.limit).unwrap_or(0);
        let mut facts = Vec::with_capacity(want);
        if want == 0 {
            return Ok(FactPage { facts, next: None });
        }
        let mut after = filter
            .after
            .as_deref()
            .map(|token| FactCursor::decode(token, &caller.user_id))
            .transpose()?;
        let held = held::build(&caller.user_id, caller.project.as_deref(), &self.audiences).await;
        loop {
            let mut batch = self
                .repos
                .facts
                .list_reachable(filter, &held.keys, after.as_ref())
                .await?;
            batch.sort_by(|a, b| (b.updated_at, b.id.as_str()).cmp(&(a.updated_at, a.id.as_str())));
            let exhausted = batch.len() < want;
            let Some(last) = batch.last().map(FactCursor::at) else {
                return Ok(FactPage { facts, next: None });
            };
            let visible = self.retain_visible(caller, batch, held.clone()).await?;
            for fact in visible {
                facts.push(fact);
                if facts.len() == want {
                    let at = facts.last().map(FactCursor::at);
                    let next = if exhausted && at.as_ref() == Some(&last) {
                        None
                    } else {
                        at.map(|cursor| cursor.encode(&caller.user_id))
                    };
                    return Ok(FactPage { facts, next });
                }
            }
            if exhausted {
                return Ok(FactPage { facts, next: None });
            }
            after = Some(last);
        }
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
        let withheld = lifecycle_withheld(&fact, ctx.as_of, inject_stale);
        Ok(FactWhy {
            fact,
            terms,
            withheld,
        })
    }
}
