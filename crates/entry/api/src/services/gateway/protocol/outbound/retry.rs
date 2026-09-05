//! Bounded retry for transient upstream gateway failures.
//!
//! Upstream capacity errors — HTTP 429 and 503 — are not statements about the
//! request, they are statements about the moment. [`send_with_retry`] re-sends
//! the same request a bounded number of times with exponential backoff and
//! jitter, honouring a `retry-after` header when the provider supplies one.
//! Every other status, and the final 429/503 once the budget is spent, is
//! handed back untouched so the gateway's wire contract still relays upstream
//! errors verbatim.
//!
//! Retry is safe here because it happens strictly before a response exists:
//! nothing has been relayed to the client, and neither the buffered nor the
//! streaming path has begun.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::cell::Cell;
use std::future::Future;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};

use super::UpstreamError;

// Why: one series for every transient upstream retry, labelled by provider and
// by the status that triggered it, so a capacity blip is visible in Prometheus
// without reading logs.
const RETRIES_TOTAL: &str = "gateway_upstream_retries_total";

// Why: four attempts spends at most ~7s of backoff before giving up, which
// rides out a Vertex MaaS capacity blip without outliving a client's patience.
pub const MAX_ATTEMPTS: u32 = 4;

// Why: the backoff shape inherited from the proxy this gateway replaced —
// 1s doubling to a 30s ceiling — kept `deepseek.v3.2` alive under load.
const BASE_DELAY_MS: u64 = 1_000;
const MAX_DELAY_MS: u64 = 30_000;

// Why: without spread, every client that got a 429 from one overloaded
// upstream retries in the same millisecond and reproduces the overload.
const JITTER_RATIO: f64 = 0.25;

/// How hard, and how patiently, a transient upstream failure is retried.
///
/// [`RetryPolicy::immediate`] keeps the attempt budget but removes every wait,
/// so tests exercise the loop in milliseconds.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter_ratio: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: MAX_ATTEMPTS,
            base_delay: Duration::from_millis(BASE_DELAY_MS),
            max_delay: Duration::from_millis(MAX_DELAY_MS),
            jitter_ratio: JITTER_RATIO,
        }
    }
}

impl RetryPolicy {
    #[must_use]
    pub fn immediate() -> Self {
        Self {
            base_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
            jitter_ratio: 0.0,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            ..Self::immediate()
        }
    }
}

// Why: only capacity signals are safe to repeat. A 4xx other than 429 is a
// verdict on the request itself, and a 500 may already have had an effect
// upstream, so replaying either would be wrong rather than merely wasteful.
#[must_use]
pub const fn is_retryable(status: u16) -> bool {
    status == 429 || status == 503
}

// Why: attempts are 1-based, so the first wait is exactly `base_delay` and
// each later one doubles until `max_delay` clamps it.
#[must_use]
pub fn backoff_delay(attempt: u32, policy: &RetryPolicy) -> Duration {
    let base = policy.base_delay.as_millis().min(u128::from(u64::MAX)) as u64;
    if base == 0 {
        return Duration::ZERO;
    }
    let shift = attempt.saturating_sub(1).min(u32::BITS - 1);
    let raw = base.saturating_mul(1u64.checked_shl(shift).unwrap_or(u64::MAX));
    let capped = raw.min(policy.max_delay.as_millis().min(u128::from(u64::MAX)) as u64);
    Duration::from_millis(apply_jitter(capped, policy.jitter_ratio))
}

// Why: spread is symmetric around the computed delay, so the average pacing
// stays the doubling curve while no two callers land on the same instant.
fn apply_jitter(millis: u64, ratio: f64) -> u64 {
    if millis == 0 || ratio <= 0.0 {
        return millis;
    }
    let spread = (millis as f64 * ratio).round() as u64;
    if spread == 0 {
        return millis;
    }
    let low = millis.saturating_sub(spread);
    let high = millis.saturating_add(spread);
    rand::random_range(low..=high)
}

// Why: RFC 9110 allows `retry-after` to be either a count of seconds or an
// HTTP-date, and providers use both. A date already in the past yields
// `Duration::ZERO` rather than `None` — the provider answered, it simply
// answered "now".
#[must_use]
pub fn parse_retry_after(value: &str, now: DateTime<Utc>) -> Option<Duration> {
    let trimmed = value.trim();
    if let Ok(seconds) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let at = DateTime::parse_from_rfc2822(trimmed)
        .map(|d| d.with_timezone(&Utc))
        .ok()?;
    let delta = at.signed_duration_since(now);
    Some(delta.to_std().unwrap_or(Duration::ZERO))
}

// Why: the provider knows its own recovery window better than our curve does,
// but only in one direction — a longer `retry-after` is obeyed, a shorter one
// would let us hammer an upstream that is already shedding load. The result is
// still clamped to `max_delay` so one hostile header cannot stall a request.
#[must_use]
pub fn effective_delay(
    attempt: u32,
    retry_after: Option<&str>,
    policy: &RetryPolicy,
    now: DateTime<Utc>,
) -> Duration {
    let computed = backoff_delay(attempt, policy);
    let Some(requested) = retry_after.and_then(|v| parse_retry_after(v, now)) else {
        return computed;
    };
    requested.clamp(computed, policy.max_delay.max(computed))
}

tokio::task_local! {
    // Why: scoping the policy to the task lets a caller — a route with its own
    // patience budget, or a test that cannot afford real seconds — override it
    // without threading a parameter through every adapter's `send`.
    static POLICY: RetryPolicy;

    // Why: `send_checked` keeps its one-value signature, so the retry count
    // rides back to whoever scoped the observation rather than the call site.
    static OBSERVED: Cell<u32>;
}

// Why: scopes `policy` over every retryable send `fut` makes.
pub async fn with_policy<F: Future>(policy: RetryPolicy, fut: F) -> F::Output {
    POLICY.scope(policy, fut).await
}

// Why: returns `fut`'s output paired with the total re-sends underneath it,
// which is the only way a caller learns a request was retried at all.
pub async fn observing_retries<F: Future>(fut: F) -> (F::Output, u32) {
    let out = OBSERVED.scope(Cell::new(0), async move {
        let out = fut.await;
        (out, OBSERVED.with(Cell::get))
    });
    out.await
}

// Why: an unscoped call is the production default, not an error.
#[must_use]
pub fn current_policy() -> RetryPolicy {
    POLICY.try_with(|p| *p).unwrap_or_default()
}

// Why: counted as each retry is decided, not from the loop's return value —
// a request that exhausts its budget still retried, and an error path that
// forgot to report those would make the worst outages look retry-free.
fn record_retry() {
    // Why: an absent scope means nobody asked for the count, which is the
    // production default rather than a failure — so it is traced, not raised.
    if OBSERVED
        .try_with(|c| c.set(c.get().saturating_add(1)))
        .is_err()
    {
        tracing::trace!("upstream retry outside an observing scope; not counted");
    }
}

// Why: returns the first successful response paired with the retry count, zero
// when the first attempt succeeded. Any non-retryable status, and the last
// retryable one once the budget is spent, becomes an `UpstreamError` carrying
// the upstream body and headers unchanged, so the gateway still relays
// upstream errors verbatim.
pub async fn send_with_retry(
    provider: &str,
    req: reqwest::RequestBuilder,
    policy: &RetryPolicy,
) -> Result<(reqwest::Response, u32)> {
    let mut attempt = 1;
    loop {
        let Some(this_try) = req.try_clone() else {
            // Why: a streaming request body cannot be replayed, so the only
            // honest thing left is a single attempt with no retry.
            return match send_once(provider, req).await {
                Ok(response) => Ok((response, attempt - 1)),
                Err(failure) => Err(into_error(provider, failure).await),
            };
        };
        let outcome = send_once(provider, this_try).await;
        let response = match outcome {
            Ok(response) => return Ok((response, attempt - 1)),
            Err(SendFailure::Fatal(e)) => return Err(e),
            Err(SendFailure::Upstream(response)) => response,
        };
        if attempt >= policy.max_attempts {
            return Err(anyhow::Error::new(
                UpstreamError::from_response(provider, response).await,
            ));
        }
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned);
        let delay = effective_delay(attempt, retry_after.as_deref(), policy, Utc::now());
        tracing::warn!(
            provider,
            status = response.status().as_u16(),
            attempt,
            max_attempts = policy.max_attempts,
            delay_ms = delay.as_millis() as u64,
            retry_after = retry_after.as_deref().unwrap_or(""),
            "Gateway upstream returned a transient failure; retrying"
        );
        record_retry();
        metrics::counter!(
            RETRIES_TOTAL,
            "provider" => provider.to_owned(),
            "status" => response.status().as_u16().to_string(),
        )
        .increment(1);
        drop(response);
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        attempt += 1;
    }
}

// Why: the loop keeps a retryable response alive to read its headers, so the
// conversion to a relayable error is deferred to whoever gives up on it.
async fn into_error(provider: &str, failure: SendFailure) -> anyhow::Error {
    match failure {
        SendFailure::Fatal(e) => e,
        SendFailure::Upstream(response) => {
            anyhow::Error::new(UpstreamError::from_response(provider, response).await)
        },
    }
}

enum SendFailure {
    Fatal(anyhow::Error),
    Upstream(reqwest::Response),
}

// Why: a retryable status must stay an unread `Response` so the next loop turn
// can read its `retry-after`, and the final one can still be relayed verbatim.
async fn send_once(
    provider: &str,
    req: reqwest::RequestBuilder,
) -> std::result::Result<reqwest::Response, SendFailure> {
    let response = req.send().await.map_err(|e| {
        SendFailure::Fatal(anyhow::Error::new(UpstreamError::Transport {
            provider: provider.to_owned(),
            source: e,
        }))
    })?;
    let status = response.status().as_u16();
    if response.status().is_success() {
        return Ok(response);
    }
    if is_retryable(status) {
        return Err(SendFailure::Upstream(response));
    }
    Err(SendFailure::Fatal(anyhow::Error::new(
        UpstreamError::from_response(provider, response).await,
    )))
}
