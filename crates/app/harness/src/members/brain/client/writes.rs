//! Changing the brain: entering a fact, governing one (confirm, dispute,
//! retract, erase, pin, unpin, field edits), resolving a dispute and deciding
//! or replaying a proposal. Which of these an account may do is the
//! instance's decision; a refusal is [`BrainError::Forbidden`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Method;
use serde_json::{Map, Value};
use sp_protocol::wire::brain::{
    BrainDisputeId, BrainFactId, BrainProposalId, DisputeCreated, DisputeRequest, EnterAccepted,
    EnteredFact, EventOutboxId, FactRevision, FieldsRequest, ReplayReport, ReplayRequest,
    ResolveRequest, RetractRequest,
};
use sp_protocol::wire::paths;

use super::BrainClient;
use crate::members::brain::error::BrainError;
use crate::members::brain::http::Call;

const ENTER: &str = "POST /api/v1/brain/facts";
const CONFIRM: &str = "POST /api/v1/brain/facts/{id}/confirm";
const DISPUTE: &str = "POST /api/v1/brain/facts/{id}/dispute";
const RETRACT: &str = "POST /api/v1/brain/facts/{id}/retract";
const ERASE: &str = "POST /api/v1/brain/facts/{id}/erase";
const PIN: &str = "POST /api/v1/brain/facts/{id}/pin";
const UNPIN: &str = "POST /api/v1/brain/facts/{id}/unpin";
const FIELDS: &str = "PATCH /api/v1/brain/facts/{id}/fields";
const RESOLVE: &str = "POST /api/v1/brain/disputes/{id}/resolve";
const APPROVE: &str = "POST /api/v1/brain/proposals/{id}/approve";
const REJECT: &str = "POST /api/v1/brain/proposals/{id}/reject";
const REPLAY: &str = "POST /api/v1/brain/proposals/{id}/replay";

impl BrainClient {
    pub async fn enter(&self, fact: &EnteredFact) -> Result<EventOutboxId, BrainError> {
        let url = self.url(paths::BRAIN_FACTS, &[])?;
        let call = Call::new(Method::POST, url, ENTER).json(fact)?;
        let accepted: EnterAccepted = self.json(call, 202).await?;
        Ok(accepted.outbox_id)
    }

    pub async fn confirm(&self, id: &BrainFactId) -> Result<FactRevision, BrainError> {
        self.revise(id, "confirm", CONFIRM).await
    }

    pub async fn pin(&self, id: &BrainFactId) -> Result<FactRevision, BrainError> {
        self.revise(id, "pin", PIN).await
    }

    pub async fn unpin(&self, id: &BrainFactId) -> Result<FactRevision, BrainError> {
        self.revise(id, "unpin", UNPIN).await
    }

    pub async fn retract(
        &self,
        id: &BrainFactId,
        reason: &str,
    ) -> Result<FactRevision, BrainError> {
        let url = self.url(paths::BRAIN_FACTS, &[id.as_str(), "retract"])?;
        let body = RetractRequest {
            reason: reason.to_owned(),
        };
        let call = Call::new(Method::POST, url, RETRACT).json(&body)?;
        self.json(call, 200).await
    }

    pub async fn erase(&self, id: &BrainFactId) -> Result<(), BrainError> {
        let url = self.url(paths::BRAIN_FACTS, &[id.as_str(), "erase"])?;
        self.answer(Call::new(Method::POST, url, ERASE), 204)
            .await?;
        Ok(())
    }

    // JSON: fact fields are typed by the fact's kind declaration, which only the
    // instance holds.
    pub async fn edit_fields(
        &self,
        id: &BrainFactId,
        fields: Map<String, Value>,
    ) -> Result<FactRevision, BrainError> {
        let url = self.url(paths::BRAIN_FACTS, &[id.as_str(), "fields"])?;
        let call = Call::new(Method::PATCH, url, FIELDS).json(&FieldsRequest { fields })?;
        self.json(call, 200).await
    }

    pub async fn dispute(
        &self,
        id: &BrainFactId,
        reason: &str,
    ) -> Result<BrainDisputeId, BrainError> {
        let url = self.url(paths::BRAIN_FACTS, &[id.as_str(), "dispute"])?;
        let body = DisputeRequest {
            reason: reason.to_owned(),
        };
        let call = Call::new(Method::POST, url, DISPUTE).json(&body)?;
        let created: DisputeCreated = self.json(call, 201).await?;
        Ok(created.dispute_id)
    }

    pub async fn resolve_dispute(
        &self,
        dispute: &BrainDisputeId,
        resolution: &ResolveRequest,
    ) -> Result<(), BrainError> {
        let id = dispute.to_string();
        let url = self.url(paths::BRAIN_DISPUTES, &[id.as_str(), "resolve"])?;
        let call = Call::new(Method::POST, url, RESOLVE).json(resolution)?;
        self.answer(call, 204).await?;
        Ok(())
    }

    pub async fn approve(&self, proposal: &BrainProposalId) -> Result<(), BrainError> {
        self.decide(proposal, "approve", APPROVE).await
    }

    pub async fn reject(&self, proposal: &BrainProposalId) -> Result<(), BrainError> {
        self.decide(proposal, "reject", REJECT).await
    }

    pub async fn replay(
        &self,
        proposal: &BrainProposalId,
        window: &ReplayRequest,
    ) -> Result<ReplayReport, BrainError> {
        let id = proposal.to_string();
        let url = self.url(paths::BRAIN_PROPOSALS, &[id.as_str(), "replay"])?;
        let call = Call::new(Method::POST, url, REPLAY).json(window)?;
        self.json(call, 200).await
    }

    async fn revise(
        &self,
        id: &BrainFactId,
        action: &str,
        endpoint: &'static str,
    ) -> Result<FactRevision, BrainError> {
        let url = self.url(paths::BRAIN_FACTS, &[id.as_str(), action])?;
        self.json(Call::new(Method::POST, url, endpoint), 200).await
    }

    async fn decide(
        &self,
        proposal: &BrainProposalId,
        action: &str,
        endpoint: &'static str,
    ) -> Result<(), BrainError> {
        let id = proposal.to_string();
        let url = self.url(paths::BRAIN_PROPOSALS, &[id.as_str(), action])?;
        self.answer(Call::new(Method::POST, url, endpoint), 204)
            .await?;
        Ok(())
    }
}
