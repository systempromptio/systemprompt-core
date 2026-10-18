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
//! [`RateWindowRepository`] is also the counter behind the gateway's
//! per-user and per-key request-rate windows.
//!
//! Every [`PRUNE_EVERY_CHARGES`]th charge through a repository prunes expired
//! windows and charges, whichever enforcement point charged it.
//!
//! Charging is idempotent per call id: a re-evaluated call reads the position
//! it was first charged at. [`RateWindowRepository::refund`] takes a call's
//! charge back out of the window it landed in, for a request a later stage
//! refused before it ran. A window whose charge fails is absent from
//! [`RateCounts`], and the policy treats an absent count as a violation —
//! fail-closed under `enforce`, a warning under `warn`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use sqlx::PgPool;
use systemprompt_identifiers::{CallId, PolicyId};
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
    pub subject: &'a str,
    pub call_id: &'a CallId,
}

impl RateCharge<'_> {
    fn bucket_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.policy_id.as_str(),
            self.window_secs,
            self.subject
        )
    }
}

pub const PRUNE_EVERY_CHARGES: u64 = 1024;

#[derive(Debug, Clone)]
pub struct RateWindowRepository {
    pool: PgPool,
    charges: Arc<AtomicU64>,
}

impl RateWindowRepository {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            charges: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn charge(&self, charge: RateCharge<'_>) -> Result<u64, RepositoryError> {
        let position = self.charge_window(charge).await?;
        if self
            .charges
            .fetch_add(1, Ordering::Relaxed)
            .is_multiple_of(PRUNE_EVERY_CHARGES)
            && let Err(error) = self.prune_expired().await
        {
            tracing::warn!(%error, "expired rate windows could not be pruned");
        }
        Ok(position)
    }

    async fn charge_window(&self, charge: RateCharge<'_>) -> Result<u64, RepositoryError> {
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

    pub async fn refund(&self, charge: RateCharge<'_>) -> Result<bool, RepositoryError> {
        let refunded = sqlx::query!(
            r#"
            WITH removed AS (
                DELETE FROM governance_rate_charges
                WHERE call_id = $1 AND bucket_key = $2
                RETURNING expires_at
            )
            UPDATE governance_rate_windows w
            SET hits = GREATEST(w.hits - 1, 0)
            FROM removed r
            WHERE w.bucket_key = $2 AND w.expires_at = r.expires_at
            "#,
            charge.call_id.as_str(),
            charge.bucket_key(),
        )
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(refunded > 0)
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
