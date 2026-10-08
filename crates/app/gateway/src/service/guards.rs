//! Pre-dispatch quota and extension guard enforcement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::QuotaFaultMode;

use super::super::protocol::canonical::CanonicalRequest;
use super::super::{GatewayAudit, GatewayRepositories, quota};
use super::resolve::ResolvedUpstream;
use super::stages::record_quota_warning;
use super::{DispatchError, GatewayError, GuardForbidden, GuardUnavailable, QuotaExceeded};
use crate::policies::{GatewayPolicySpec, QuotaWindow};

#[derive(Debug, Clone, Copy)]
pub(super) struct QuotaAdmission<'a> {
    pub(super) policy: &'a GatewayPolicySpec,
    pub(super) fault_mode: QuotaFaultMode,
    pub(super) estimate: quota::QuotaEstimate,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Priced<'a> {
    pub(super) raw_body_len: usize,
    pub(super) pricing: &'a systemprompt_manifest::services::ModelPricing,
}

impl<'a> QuotaAdmission<'a> {
    pub(super) fn for_request(
        policy: &'a GatewayPolicySpec,
        fault_mode: QuotaFaultMode,
        upstream: &ResolvedUpstream<'_>,
        request: &CanonicalRequest,
        priced: Priced<'_>,
    ) -> Self {
        let model_limits = upstream
            .provider
            .find_served_model(request.model.as_str())
            .map(|m| m.limits);
        Self {
            policy,
            fault_mode,
            estimate: quota::estimate(
                priced.raw_body_len,
                request.max_tokens,
                model_limits.as_ref(),
                priced.pricing,
            ),
        }
    }
}

pub(super) async fn enforce_quota(
    db: &DbPool,
    repos: &GatewayRepositories,
    audit: &GatewayAudit,
    admission: QuotaAdmission<'_>,
) -> Result<(), DispatchError> {
    let QuotaAdmission {
        policy,
        fault_mode,
        estimate,
    } = admission;
    let ctx = &audit.ctx;
    let windows: Vec<QuotaWindow> = policy
        .quota_windows
        .iter()
        .chain(&ctx.api_key_windows)
        .cloned()
        .collect();
    let outcome = quota::precheck_and_reserve(
        &repos.quota_buckets,
        quota::ReserveParams {
            providers: &repos.subject_providers,
            subjects: quota::QuotaSubjects {
                user_id: &ctx.user_id,
                api_key_id: ctx.attribution.api_key_id.as_ref(),
                attribution: &ctx.attribution,
            },
            windows: &windows,
            fault_mode,
            estimate,
        },
    )
    .await
    .map_err(|e| DispatchError::Recorded(GatewayError::internal("quota precheck failed", e)))?;
    let (decision, reservation) = match outcome {
        quota::ReserveOutcome::Admitted(reservation) => {
            audit.set_quota_reservation(reservation);
            return Ok(());
        },
        quota::ReserveOutcome::Denied {
            decision,
            reservation,
        } => (decision, reservation),
    };
    count_denial(&decision, policy);
    if policy.quota_mode.is_warn() {
        audit.set_quota_reservation(reservation);
        tracing::warn!(
            ai_request_id = %ctx.ai_request_id,
            user_id = %ctx.user_id,
            window_seconds = decision.window_seconds,
            reason = %decision.message,
            "Gateway quota window exhausted in warn mode; allowing the request"
        );
        record_quota_warning(db, ctx, &decision.message)
            .await
            .map_err(|e| {
                DispatchError::Recorded(GatewayError::internal("quota warning record failed", e))
            })?;
        return Ok(());
    }
    audit.set_quota_reservation(reservation);
    let msg = decision.message;
    if let Err(e) = audit.fail(&msg).await {
        tracing::warn!(error = %e, "quota audit fail failed");
    }
    Err(DispatchError::recorded(QuotaExceeded {
        message: msg,
        retry_after_seconds: i32::try_from(decision.detail.retry_after_seconds).unwrap_or(i32::MAX),
        detail: Some(decision.detail),
    }))
}

fn count_denial(decision: &quota::QuotaDecision, policy: &GatewayPolicySpec) {
    let mode = if policy.quota_mode.is_warn() {
        "warn"
    } else {
        "enforce"
    };
    metrics::counter!(
        "systemprompt_quota_denials_total",
        "subject_kind" => decision.detail.subject.clone(),
        "dimension" => decision.detail.dimension.map_or("unevaluated", quota::QuotaDimension::as_str),
        "mode" => mode,
    )
    .increment(1);
}

pub(super) async fn enforce_request_guards(
    db: &DbPool,
    user_id: &UserId,
    upstream: &ResolvedUpstream<'_>,
    request: &CanonicalRequest,
    audit: &GatewayAudit,
) -> Result<(), DispatchError> {
    if systemprompt_extension::gateway_guards().is_empty() {
        return Ok(());
    }
    let route_id = upstream.route.effective_id();
    let guard_request = systemprompt_extension::GatewayGuardRequest {
        user_id,
        model: &request.model,
        route_id: Some(&route_id),
        provider: &upstream.route.provider,
        streaming: request.stream,
        attribution: &audit.ctx.attribution,
        api_key_id: audit.ctx.attribution.api_key_id.as_ref(),
    };
    let outcome = if db.pool().is_closed() {
        Err(systemprompt_extension::GatewayDenyReason::unavailable(
            "Request guards require a database connection",
        ))
    } else {
        systemprompt_extension::run_gateway_guards(db.as_ref(), &guard_request).await
    };
    let Err(deny) = outcome else {
        return Ok(());
    };
    tracing::warn!(
        user_id = %user_id,
        model = %request.model,
        route_id = %route_id,
        kind = ?deny.kind,
        reason = %deny.message,
        "Gateway request denied by request guard"
    );
    if let Err(e) = audit.fail(&deny.message).await {
        tracing::warn!(error = %e, "request-guard audit fail failed");
    }
    let inner: GatewayError = match deny.kind {
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
            detail: None,
        }
        .into(),
    };
    Err(DispatchError::Recorded(inner))
}
