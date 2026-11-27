//! What a person does with the brain from the shell, and what the agent tools
//! call: reads, governing a fact, deciding a proposal and entering a fact.
//! Each goes through [`BrainService::call`], so it is refused without a
//! request unless the brain is enabled, and a change bumps the feed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{BrainDisputeId, BrainFactId, BrainProposalId, EventOutboxId};
use systemprompt_models::brain::client::{FactPage, FactWhy};
use systemprompt_models::brain::{BrainFact, BrainProposal, EnteredFact, FactRevision, SubjectKey};

use super::{BrainService, Change};
use crate::members::brain::client::FactQuery;
use crate::members::brain::error::BrainError;

impl BrainService {
    pub async fn facts(&self, query: &FactQuery) -> Result<FactPage, BrainError> {
        self.call(Change::Nothing, |client| async move {
            client.facts(query).await
        })
        .await
    }

    pub async fn fact(&self, id: &BrainFactId) -> Result<BrainFact, BrainError> {
        self.call(
            Change::Nothing,
            |client| async move { client.fact(id).await },
        )
        .await
    }

    pub async fn timeline(&self, id: &BrainFactId) -> Result<Vec<FactRevision>, BrainError> {
        self.call(Change::Nothing, |client| async move {
            client.timeline(id).await
        })
        .await
    }

    pub async fn why(
        &self,
        id: &BrainFactId,
        project: Option<&str>,
    ) -> Result<FactWhy, BrainError> {
        self.call(Change::Nothing, |client| async move {
            client.why(id, project).await
        })
        .await
    }

    pub async fn subject(
        &self,
        subject: &SubjectKey,
        limit: Option<u32>,
        after: Option<&str>,
    ) -> Result<FactPage, BrainError> {
        self.call(Change::Nothing, |client| async move {
            client.subject(subject, limit, after).await
        })
        .await
    }

    pub async fn proposals(&self, limit: Option<u32>) -> Result<Vec<BrainProposal>, BrainError> {
        self.call(Change::Nothing, |client| async move {
            client.proposals(limit).await
        })
        .await
    }

    pub async fn enter(&self, fact: &EnteredFact) -> Result<EventOutboxId, BrainError> {
        self.call(
            Change::Facts,
            |client| async move { client.enter(fact).await },
        )
        .await
    }

    pub async fn confirm(&self, id: &BrainFactId) -> Result<FactRevision, BrainError> {
        self.call(
            Change::Facts,
            |client| async move { client.confirm(id).await },
        )
        .await
    }

    pub async fn dispute(
        &self,
        id: &BrainFactId,
        reason: &str,
    ) -> Result<BrainDisputeId, BrainError> {
        self.call(Change::Facts, |client| async move {
            client.dispute(id, reason).await
        })
        .await
    }

    pub async fn retract(
        &self,
        id: &BrainFactId,
        reason: &str,
    ) -> Result<FactRevision, BrainError> {
        self.call(Change::Facts, |client| async move {
            client.retract(id, reason).await
        })
        .await
    }

    pub async fn pin(&self, id: &BrainFactId) -> Result<FactRevision, BrainError> {
        self.call(Change::Facts, |client| async move { client.pin(id).await })
            .await
    }

    pub async fn unpin(&self, id: &BrainFactId) -> Result<FactRevision, BrainError> {
        self.call(
            Change::Facts,
            |client| async move { client.unpin(id).await },
        )
        .await
    }

    pub async fn approve(&self, proposal: &BrainProposalId) -> Result<(), BrainError> {
        self.call(Change::Proposals, |client| async move {
            client.approve(proposal).await
        })
        .await
    }

    pub async fn reject(&self, proposal: &BrainProposalId) -> Result<(), BrainError> {
        self.call(Change::Proposals, |client| async move {
            client.reject(proposal).await
        })
        .await
    }
}
