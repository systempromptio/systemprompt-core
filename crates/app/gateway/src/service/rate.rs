//! Per-user and per-API-key request-rate windows on gateway admission.
//!
//! Each configured [`RateWindow`] charges a fixed window in Postgres keyed by
//! its subject, so the limit holds across every replica. A request past
//! `max_requests` is refused with 429 and a `Retry-After` naming the end of
//! the window, whatever the policy's `quota_mode`. A request authenticated
//! without an API key is not charged against `api_key` windows. A counter that
//! cannot be charged follows `quota_fault_mode`: `closed` refuses with 503,
//! `open` admits and logs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::{CallId, PolicyId};
use systemprompt_manifest::services::QuotaFaultMode;
use systemprompt_security::policy::RateCharge;

use super::{DispatchError, GatewayError, GuardUnavailable, QuotaExceeded};
use crate::GatewayRepositories;
use crate::audit::GatewayAudit;
use crate::policies::{GatewayPolicySpec, RateSubject, RateWindow};

const RATE_POLICY_PREFIX: &str = "gateway_rate";
const UNAVAILABLE_RETRY_SECONDS: i32 = 1;

pub(super) async fn enforce_rate_windows(
    repos: &GatewayRepositories,
    audit: &GatewayAudit,
    policy: &GatewayPolicySpec,
    fault_mode: QuotaFaultMode,
) -> Result<(), DispatchError> {
    let ctx = &audit.ctx;
    let call = CallId::new(ctx.ai_request_id.as_str());
    for window in &policy.rate_windows {
        let subject = match window.subject {
            RateSubject::User => ctx.user_id.as_str().to_owned(),
            RateSubject::ApiKey => match ctx.attribution.api_key_id.as_ref() {
                Some(key) => key.as_str().to_owned(),
                None => continue,
            },
        };
        let policy_id = PolicyId::new(format!("{RATE_POLICY_PREFIX}_{}", window.subject.as_str()));
        let charged = repos
            .rate_windows
            .charge(RateCharge {
                policy_id: &policy_id,
                window_secs: u64::from(window.window_seconds),
                subject: &subject,
                call_id: &call,
            })
            .await;
        match charged {
            Ok(position) if position > u64::from(window.max_requests) => {
                return Err(refuse(audit, window).await);
            },
            Ok(_) => {},
            Err(error) if fault_mode.is_closed() => {
                tracing::error!(%error, "gateway rate window could not be charged; refusing");
                return Err(DispatchError::recorded(GuardUnavailable {
                    message: "Request-rate limiter unavailable".to_owned(),
                    retry_after_seconds: UNAVAILABLE_RETRY_SECONDS,
                }));
            },
            Err(error) => {
                tracing::warn!(%error, "gateway rate window could not be charged; admitting");
            },
        }
    }
    Ok(())
}

async fn refuse(audit: &GatewayAudit, window: &RateWindow) -> DispatchError {
    let message = format!(
        "Request rate exceeded: {} requests per {}s per {}",
        window.max_requests,
        window.window_seconds,
        window.subject.as_str()
    );
    if let Err(error) = audit.fail(&message).await {
        tracing::warn!(%error, "rate window audit fail failed");
    }
    DispatchError::Recorded(GatewayError::from(QuotaExceeded {
        message,
        retry_after_seconds: seconds_until_window_end(window.window_seconds),
        detail: None,
    }))
}

fn seconds_until_window_end(window_seconds: u32) -> i32 {
    let window = i64::from(window_seconds.max(1));
    let elapsed = Utc::now().timestamp().rem_euclid(window);
    i32::try_from(window - elapsed).unwrap_or(i32::MAX)
}
