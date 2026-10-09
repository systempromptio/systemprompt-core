//! The admission write for a gateway request, in one transaction.
//!
//! Everything the gateway records about a request before it calls upstream
//! (the request row and its attributions, client evidence, the request
//! payload and offered tools, the canonical messages, the route and served
//! provider, the system-prompt override, the prepared-body digest and the
//! request-phase safety findings) commits together. The fields that were
//! best-effort before stay best-effort: each runs under its own savepoint, so a
//! failure is logged and rolled back without losing the request row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;
use sqlx::{Acquire, Postgres, Transaction};
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::origin::ClientEvidence;
use systemprompt_traits::RepositoryError;

use super::AiRequestRepository;
use super::attributions::insert_attributions;
use super::message_operations::{RequestMessageRow, insert_messages_with};
use super::mutations::{
    insert_request_row, update_route_match_with, update_served_provider_with,
    update_system_prompt_override_with,
};
use crate::models::AiRequestRecord;
use crate::repository::ai_request_client_evidence::upsert_with as upsert_evidence_with;
use crate::repository::ai_request_payloads::{
    UpsertPayloadParams, upsert_offered_tools_with, upsert_prepared_with, upsert_request_with,
};
use crate::repository::ai_safety_findings::{InsertSafetyFinding, insert_many_with};

#[derive(Debug, Clone, Copy)]
pub struct PreparedDigest<'a> {
    pub sha256: &'a str,
    pub tools: Option<&'a Value>,
}

#[derive(Debug, Clone, Copy)]
pub struct AdmissionWrite<'a> {
    pub id: &'a AiRequestId,
    pub record: &'a AiRequestRecord,
    pub evidence: &'a ClientEvidence,
    pub payload: UpsertPayloadParams<'a>,
    pub offered_tools: Option<&'a Value>,
    pub messages: &'a [RequestMessageRow<'a>],
    pub route_match: Option<&'a str>,
    pub served_provider: Option<&'a str>,
    pub system_prompt_override: Option<&'a str>,
    pub prepared: Option<PreparedDigest<'a>>,
    pub findings: &'a [InsertSafetyFinding<'a>],
}

impl AiRequestRepository {
    pub async fn admit(&self, write: AdmissionWrite<'_>) -> Result<(), RepositoryError> {
        let mut tx = self.write_pool().begin().await?;
        if !insert_request_row(&mut tx, write.id, write.record).await? {
            return Err(RepositoryError::conflict(
                "AI request",
                write.id,
                "already exists",
            ));
        }
        insert_attributions(&mut tx, write.id, &write.record.attribution).await?;
        upsert_evidence_with(&mut *tx, write.id, write.evidence).await?;
        upsert_request_with(&mut *tx, write.id, write.payload).await?;
        if let Some(tools) = write.offered_tools {
            upsert_offered_tools_with(&mut *tx, write.id, tools).await?;
        }
        insert_messages_with(&mut *tx, write.id, write.messages).await?;
        admit_best_effort(&mut tx, &write).await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn admit_best_effort(
    tx: &mut Transaction<'_, Postgres>,
    write: &AdmissionWrite<'_>,
) -> Result<(), RepositoryError> {
    if let Some(descriptor) = write.route_match {
        let mut sp = tx.begin().await?;
        let outcome = update_route_match_with(&mut *sp, write.id, descriptor).await;
        settle_savepoint(sp, outcome, "update_route_match failed").await?;
    }
    if let Some(provider) = write.served_provider {
        let mut sp = tx.begin().await?;
        let outcome = update_served_provider_with(&mut *sp, write.id, provider).await;
        settle_savepoint(sp, outcome, "update_served_provider failed").await?;
    }
    if let Some(descriptor) = write.system_prompt_override {
        let mut sp = tx.begin().await?;
        let outcome = update_system_prompt_override_with(&mut *sp, write.id, descriptor).await;
        settle_savepoint(sp, outcome, "update_system_prompt_override failed").await?;
    }
    if let Some(prepared) = write.prepared {
        let mut sp = tx.begin().await?;
        let outcome =
            upsert_prepared_with(&mut *sp, write.id, prepared.sha256, prepared.tools).await;
        settle_savepoint(sp, outcome, "prepared body digest write failed").await?;
    }
    if !write.findings.is_empty() {
        let mut sp = tx.begin().await?;
        let outcome = insert_many_with(&mut *sp, write.findings).await.map(drop);
        settle_savepoint(sp, outcome, "safety findings insert failed").await?;
    }
    Ok(())
}

async fn settle_savepoint(
    sp: Transaction<'_, Postgres>,
    outcome: Result<(), RepositoryError>,
    message: &'static str,
) -> Result<(), RepositoryError> {
    match outcome {
        Ok(()) => sp.commit().await?,
        Err(error) => {
            tracing::warn!(%error, "{message}");
            sp.rollback().await?;
        },
    }
    Ok(())
}
