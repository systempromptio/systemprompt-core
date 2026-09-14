//! Pre-dispatch quota and extension guard enforcement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::services::QuotaFaultMode;

use super::super::policy::GatewayPolicySpec;
use super::super::protocol::canonical::CanonicalRequest;
use super::super::{GatewayAudit, GatewayRepositories, quota};
use super::resolve::ResolvedUpstream;
use super::stages::record_quota_warning;
use super::{DispatchError, GuardForbidden, GuardUnavailable, QuotaExceeded};

pub(super) async fn enforce_quota(
    db: &DbPool,
    repos: &GatewayRepositories,
    policy: &GatewayPolicySpec,
    audit: &GatewayAudit,
    fault_mode: QuotaFaultMode,
) -> Result<(), DispatchError> {
    let ctx = &audit.ctx;
    let reservation = quota::precheck_and_reserve(
        db,
        &repos.quota_buckets,
        &ctx.user_id,
        &policy.quota_windows,
        fault_mode,
    )
    .await
    .map_err(DispatchError::Recorded)?;
    let Some(decision) = reservation else {
        return Ok(());
    };
    if decision.allow {
        return Ok(());
    }
    if policy.quota_mode.is_warn() {
        tracing::warn!(
            ai_request_id = %ctx.ai_request_id,
            user_id = %ctx.user_id,
            window_seconds = decision.window_seconds,
            reason = %decision.message,
            "Gateway quota window exhausted in warn mode; allowing the request"
        );
        record_quota_warning(db, ctx, &decision.message)
            .await
            .map_err(DispatchError::Recorded)?;
        return Ok(());
    }
    let msg = decision.message;
    if let Err(e) = audit.fail(&msg).await {
        tracing::warn!(error = %e, "quota audit fail failed");
    }
    Err(DispatchError::Recorded(
        QuotaExceeded {
            message: msg,
            retry_after_seconds: decision.window_seconds,
        }
        .into(),
    ))
}

pub(super) async fn enforce_request_guards(
    db: &DbPool,
    user_id: &UserId,
    upstream: &ResolvedUpstream<'_>,
    request: &CanonicalRequest,
    audit: &GatewayAudit,
) -> Result<(), DispatchError> {
    let Some(pool) = db.pool() else {
        return Ok(());
    };
    let guard_request = systemprompt_extension::GatewayGuardRequest {
        user_id: user_id.as_str(),
        model: &request.model,
        route_id: Some(upstream.route.id.as_str()),
        provider: upstream.route.provider.as_str(),
        streaming: request.stream,
    };
    let Err(deny) = systemprompt_extension::run_gateway_guards(&pool, &guard_request).await else {
        return Ok(());
    };
    tracing::warn!(
        user_id = %user_id,
        model = %request.model,
        route_id = %upstream.route.id,
        kind = ?deny.kind,
        reason = %deny.message,
        "Gateway request denied by request guard"
    );
    if let Err(e) = audit.fail(&deny.message).await {
        tracing::warn!(error = %e, "request-guard audit fail failed");
    }
    let inner: anyhow::Error = match deny.kind {
        systemprompt_extension::GatewayDenyKind::Unavailable => GuardUnavailable {
            message: deny.message,
            retry_after_seconds: deny.retry_after_seconds,
        }
        .into(),
        systemprompt_extension::GatewayDenyKind::Forbidden => GuardForbidden {
            message: deny.message,
        }
        .into(),
        // Why: the enum is non_exhaustive, and a denial whose kind this build
        // does not know must still deny rather than fall through to a send.
        _ => QuotaExceeded {
            message: deny.message,
            retry_after_seconds: deny.retry_after_seconds,
        }
        .into(),
    };
    Err(DispatchError::Recorded(inner))
}
