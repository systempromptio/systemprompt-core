//! When a session token is renewed, retried and considered fresh enough.
//!
//! Pure, so the timing
//! rules hold without a clock: a token is renewed at 80 % of its lifetime
//! (never sooner than 10 s, never later than 12 h from now), and a failed
//! renewal is retried every 30 s while the token still holds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use crate::client::Freshness;
use chrono::{DateTime, TimeDelta, Utc};

pub const RETRY_EVERY: Duration = Duration::from_secs(30);

const MIN_RENEWAL_WAIT: Duration = Duration::from_secs(10);

const MAX_RENEWAL_WAIT: Duration = Duration::from_hours(12);

const RENEW_AT_TENTHS: i64 = 8;

#[must_use]
pub fn renewal_delay(
    minted_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Duration {
    let lifetime_ms = expires_at
        .signed_duration_since(minted_at)
        .num_milliseconds()
        .max(0);
    let renew_after = TimeDelta::milliseconds(lifetime_ms.saturating_mul(RENEW_AT_TENTHS) / 10);
    let renew_at = minted_at
        .checked_add_signed(renew_after)
        .unwrap_or(expires_at);
    renew_at
        .signed_duration_since(now)
        .to_std()
        .unwrap_or(Duration::ZERO)
        .clamp(MIN_RENEWAL_WAIT, MAX_RENEWAL_WAIT)
}

#[must_use]
pub fn retry_delay(expires_at: DateTime<Utc>, now: DateTime<Utc>) -> Option<Duration> {
    let left = expires_at.signed_duration_since(now).to_std().ok()?;
    (!left.is_zero()).then(|| left.min(RETRY_EVERY))
}

#[must_use]
pub fn satisfies(expires_at: DateTime<Utc>, now: DateTime<Utc>, freshness: Freshness) -> bool {
    match freshness {
        Freshness::Fresh => false,
        Freshness::Any => expires_at > now,
        Freshness::ValidFor(margin) => TimeDelta::from_std(margin)
            .ok()
            .and_then(|margin| now.checked_add_signed(margin))
            .is_some_and(|needed| expires_at > needed),
    }
}
