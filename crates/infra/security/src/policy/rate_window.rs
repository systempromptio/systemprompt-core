//! Cluster-wide fixed-window counters behind the `rate_limit` policy.
//!
//! [`GovernancePolicy::evaluate`][super::GovernancePolicy::evaluate] is pure
//! and sync, so counting happens before the chain runs: the enforcement point
//! calls [`GovernanceEngine::charge_rate_windows`][super::GovernanceEngine::charge_rate_windows],
//! which charges every enabled rate window in Postgres and hands the
//! resulting [`RateCounts`] to the policy through
//! [`PolicyContext::rate_counts`][super::PolicyContext::rate_counts]. Every
//! replica charges the same row, so a budget holds per deployment.
//!
//! Charging is idempotent per call id: a re-evaluated call reads the position
//! it was first charged at. A window whose charge fails is absent from
//! [`RateCounts`], and the policy treats an absent count as a violation —
//! fail-closed under `enforce`, a warning under `warn`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_identifiers::{CallId, PolicyId, SessionId, UserId};
use systemprompt_models::errors::RepositoryError;

/// Each charged window's 1-based position of the current call within it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RateCounts {
    positions: Vec<(PolicyId, u64)>,
}

impl RateCounts {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            positions: Vec::new(),
        }
    }

    pub fn record(&mut self, policy_id: PolicyId, position: u64) {
        self.positions.retain(|(id, _)| *id != policy_id);
        self.positions.push((policy_id, position));
    }

    #[must_use]
    pub fn position(&self, policy_id: &PolicyId) -> Option<u64> {
        self.positions
            .iter()
            .find(|(id, _)| id == policy_id)
            .map(|(_, position)| *position)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RateCharge<'a> {
    pub policy_id: &'a PolicyId,
    pub window_secs: u64,
    pub session_id: &'a SessionId,
    pub user_id: &'a UserId,
    pub call_id: &'a CallId,
}

impl RateCharge<'_> {
    fn bucket_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.policy_id.as_str(),
            self.window_secs,
            self.session_id.as_str(),
            self.user_id.as_str()
        )
    }
}

#[derive(Debug, Clone)]
pub struct RateWindowRepository {
    pool: PgPool,
}

impl RateWindowRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn charge(&self, charge: RateCharge<'_>) -> Result<u64, RepositoryError> {
        let window_secs = i64::try_from(charge.window_secs.max(1)).unwrap_or(i64::MAX);
        let position = sqlx::query_scalar!(
            r#"
            WITH bounds AS (
                SELECT to_timestamp(
                    (floor(extract(epoch FROM now()) / $3::bigint) * $3::bigint)::double precision
                ) AS window_start
            ),
            existing AS (
                SELECT position FROM governance_rate_charges
                WHERE call_id = $1 AND bucket_key = $2
            ),
            bumped AS (
                INSERT INTO governance_rate_windows (bucket_key, window_start, hits, expires_at)
                SELECT $2, b.window_start, 1,
                       b.window_start + make_interval(secs => ($3::bigint * 2)::double precision)
                FROM bounds b
                WHERE NOT EXISTS (SELECT 1 FROM existing)
                ON CONFLICT (bucket_key, window_start)
                DO UPDATE SET hits = governance_rate_windows.hits + 1
                RETURNING hits, expires_at
            ),
            recorded AS (
                INSERT INTO governance_rate_charges (call_id, bucket_key, position, expires_at)
                SELECT $1, $2, hits, expires_at FROM bumped
                ON CONFLICT (call_id, bucket_key) DO NOTHING
            )
            SELECT COALESCE((SELECT position FROM existing), (SELECT hits FROM bumped))
                AS "position!"
            "#,
            charge.call_id.as_str(),
            charge.bucket_key(),
            window_secs,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(u64::try_from(position).unwrap_or(0))
    }

    pub async fn prune_expired(&self) -> Result<u64, RepositoryError> {
        let windows = sqlx::query!("DELETE FROM governance_rate_windows WHERE expires_at < now()")
            .execute(&self.pool)
            .await?
            .rows_affected();
        let charges = sqlx::query!("DELETE FROM governance_rate_charges WHERE expires_at < now()")
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(windows + charges)
    }
}
