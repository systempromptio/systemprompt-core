//! Commits a dispatch's staged admission writes before the upstream call,
//! and decorates the response with the dispatch's headers after it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::response::Response;
use systemprompt_identifiers::AiRequestId;

use super::finalize::attach_request_id;
use super::stages::{self, ScannedDispatch};
use super::{DispatchError, GatewayError};
use crate::audit::GatewayAudit;
use crate::registry::GatewayEngines;

async fn commit_admission(audit: &GatewayAudit) -> Result<(), DispatchError> {
    let Err(error) = audit.commit_admission().await else {
        return Ok(());
    };
    let error = GatewayError::internal("audit admission failed", error);
    match audit
        .fail("Gateway admission failed before provider dispatch")
        .await
    {
        Ok(()) => Err(DispatchError::Recorded(error)),
        Err(settlement_error) => {
            tracing::error!(%settlement_error, "Could not record failed gateway admission");
            Err(DispatchError::PreAudit(error))
        },
    }
}

pub(super) async fn admitted(
    audit: &GatewayAudit,
    engines: &GatewayEngines,
    scanned: &ScannedDispatch,
) -> Result<(), DispatchError> {
    commit_admission(audit).await?;
    stages::record_recall(audit, engines.memory_recall.as_ref(), scanned.memory());
    audit.record_billing_source().await;
    Ok(())
}

pub(super) fn decorate(
    mut response: Response<Body>,
    scanned: &ScannedDispatch,
    ai_request_id: &AiRequestId,
) -> Response<Body> {
    stages::recovery::attach_recovery_count(&mut response, scanned.recovery_count());
    stages::attach_memory_ids(&mut response, scanned.memory().ids_header.as_deref());
    attach_request_id(response, ai_request_id)
}
