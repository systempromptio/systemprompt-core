//! The run's personal access token.
//!
//! Issued through the users domain's [`ApiKeyService`] with the run's scope
//! bindings and ceilings, so the gateway attributes and limits every request
//! the run makes; revoked when the run ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::Utc;
use systemprompt_identifiers::{ApiKeyId, ApiKeySecret, UserId};
use systemprompt_users::{ApiKeyLimits, ApiKeyService, IssueApiKeyParams};

use super::{IssueRunCredentials, RunCredentialLimits};
use crate::error::{AgentRunsError, AgentRunsResult};

const MICRODOLLARS_PER_USD: f64 = 1_000_000.0;

pub(super) async fn issue_pat(
    keys: &ApiKeyService,
    req: &IssueRunCredentials,
) -> AgentRunsResult<(ApiKeyId, ApiKeySecret)> {
    let ttl = chrono_ttl(req.ttl)?;
    let limits = api_key_limits(req.limits, req.ttl)?;
    let issued = keys
        .issue(IssueApiKeyParams {
            user_id: &req.user_id,
            name: &req.name,
            expires_at: Some(Utc::now() + ttl),
            limits: &limits,
            scopes: &req.scopes,
            purpose: req.purpose.as_deref(),
        })
        .await
        .map_err(|source| AgentRunsError::ApiKey {
            step: "issue",
            source,
        })?;
    Ok((issued.record.id, ApiKeySecret::new(issued.secret)))
}

pub(super) async fn revoke_pat(
    keys: &ApiKeyService,
    id: &ApiKeyId,
    user_id: &UserId,
) -> AgentRunsResult<bool> {
    keys.revoke(id, user_id)
        .await
        .map_err(|source| AgentRunsError::ApiKey {
            step: "revoke",
            source,
        })
}

pub(super) fn chrono_ttl(ttl: Duration) -> AgentRunsResult<chrono::Duration> {
    chrono::Duration::from_std(ttl).map_err(|source| AgentRunsError::CredentialTtl { source })
}

fn api_key_limits(limits: RunCredentialLimits, ttl: Duration) -> AgentRunsResult<ApiKeyLimits> {
    let budget_microdollars = limits.max_budget_usd.map(microdollars).transpose()?;
    let max_requests = limits
        .max_requests
        .map(|max| i32::try_from(max).unwrap_or(i32::MAX));
    let ceiling = budget_microdollars.is_some() || max_requests.is_some();
    let window = i32::try_from(ttl.as_secs().max(1)).unwrap_or(i32::MAX);
    Ok(ApiKeyLimits {
        model_allowlist: None,
        budget_microdollars,
        max_requests,
        request_window_seconds: ceiling.then_some(window),
    })
}

fn microdollars(usd: f64) -> AgentRunsResult<i64> {
    let scaled = (usd * MICRODOLLARS_PER_USD).round();
    if !scaled.is_finite() || scaled < 0.0 || scaled > 9.0e15 {
        return Err(AgentRunsError::InvalidBudget { usd });
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the range check above bounds `scaled` to i64 without loss"
    )]
    Ok(scaled as i64)
}
