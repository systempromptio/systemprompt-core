//! Request opening: the effective policy and the audit row a dispatch starts
//! from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use bytes::Bytes;
use systemprompt_manifest::services::QuotaFaultMode;

use super::{DispatchError, GatewayError};
use crate::audit::{GatewayAudit, GatewayRequestContext};
use crate::policies::GatewayPolicySpec;
use crate::protocol::canonical::CanonicalRequest;

pub(super) async fn dispatch_policy(
    repos: &crate::GatewayRepositories,
    ctx: &GatewayRequestContext,
    fault_mode: QuotaFaultMode,
) -> Result<GatewayPolicySpec, DispatchError> {
    if ctx.session_id.is_none() {
        return Err(DispatchError::pre_audit(GatewayError::MissingSession));
    }

    let policy = repos
        .policy_resolver
        .resolve(fault_mode)
        .await
        .map_err(DispatchError::pre_audit)?;
    Ok(policy)
}

pub(super) async fn open_audit(
    repos: &crate::GatewayRepositories,
    ctx: &GatewayRequestContext,
    request: &CanonicalRequest,
    raw_body: &Bytes,
    identity_headers: &[(String, String)],
) -> Result<Arc<GatewayAudit>, DispatchError> {
    let audit = Arc::new(GatewayAudit::new(repos, ctx.clone()));
    if let Err(error) = audit.open(request, raw_body).await {
        if let Err(settlement_error) = audit
            .fail("Gateway admission failed before provider dispatch")
            .await
        {
            tracing::error!(%settlement_error, "Could not record failed gateway admission");
        }
        return Err(DispatchError::PreAudit(GatewayError::internal(
            "audit admission failed",
            error,
        )));
    }
    if !identity_headers.is_empty() {
        tracing::info!(
            ai_request_id = %ctx.ai_request_id,
            user_id = %ctx.user_id,
            headers = ?identity_headers,
            "Gateway consumed client identity headers"
        );
    }
    Ok(audit)
}
