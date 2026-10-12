//! Claim queries: `FOR UPDATE SKIP LOCKED` over queued runs, lease renewal
//! and stale-lease reclaim.
//!
//! `claim_queued` takes the oldest due queued run of an origin kind into
//! `preparing` under a lease. `renew_lease` returns `false` once the run is
//! no longer ours (reclaimed by another claimer, or terminal).
//! `reclaim_stale` re-queues an expired run below `max_attempts` with its
//! attempt bumped and `resume_session_id` set from the host session, and
//! fails the rest as `host_crashed`; it returns every reclaimed id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::HeadlessRunId;

use super::{HeadlessRunRepository, HeadlessRunRow};
use crate::error::HeadlessResult;

impl HeadlessRunRepository {
    pub async fn claim_queued(
        &self,
        origin_kind: &str,
        claimed_by: &str,
        lease: Duration,
    ) -> HeadlessResult<Option<HeadlessRunRow>> {
        let claimed = sqlx::query_scalar!(
            r#"
            UPDATE headless_runs
            SET status = 'preparing', claimed_by = $2, claimed_at = NOW(),
                lease_until = NOW() + make_interval(secs => $3::double precision)
            WHERE id = (
                SELECT id FROM headless_runs
                WHERE status = 'queued' AND origin_kind = $1
                  AND (not_before IS NULL OR not_before <= NOW())
                ORDER BY created_at
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id AS "id: HeadlessRunId"
            "#,
            origin_kind,
            claimed_by,
            lease.as_secs_f64(),
        )
        .fetch_optional(self.write_pool())
        .await?;
        match claimed {
            Some(id) => self.find_run(&id).await,
            None => Ok(None),
        }
    }

    pub async fn renew_lease(
        &self,
        id: &HeadlessRunId,
        claimed_by: &str,
        lease: Duration,
    ) -> HeadlessResult<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE headless_runs
            SET lease_until = NOW() + make_interval(secs => $3::double precision)
            WHERE id = $1 AND claimed_by = $2 AND status IN ('preparing', 'running')
            "#,
            id.as_str(),
            claimed_by,
            lease.as_secs_f64(),
        )
        .execute(self.write_pool())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn reclaim_stale(
        &self,
        origin_kind: &str,
        max_attempts: i32,
    ) -> HeadlessResult<Vec<HeadlessRunId>> {
        let ids = sqlx::query_scalar!(
            r#"
            WITH stale AS (
                SELECT id, attempt < $2 AS retry FROM headless_runs
                WHERE origin_kind = $1 AND status IN ('preparing', 'running')
                  AND lease_until < NOW()
                FOR UPDATE SKIP LOCKED
            )
            UPDATE headless_runs r
            SET status = CASE WHEN s.retry THEN 'queued' ELSE 'failed' END,
                attempt = CASE WHEN s.retry THEN r.attempt + 1 ELSE r.attempt END,
                request = CASE WHEN s.retry THEN jsonb_set(
                    CASE WHEN r.host_session_id IS NULL THEN r.request
                         ELSE jsonb_set(r.request, '{resume_session_id}', to_jsonb(r.host_session_id))
                    END,
                    '{attempt}', to_jsonb(r.attempt + 1)) ELSE r.request END,
                error_class = CASE WHEN s.retry THEN NULL ELSE 'host_crashed' END,
                error = CASE WHEN s.retry THEN NULL ELSE 'lease expired on the final attempt' END,
                finished_at = CASE WHEN s.retry THEN NULL ELSE NOW() END,
                claimed_by = NULL, claimed_at = NULL, lease_until = NULL, pid = NULL
            FROM stale s
            WHERE r.id = s.id
            RETURNING r.id AS "id: HeadlessRunId"
            "#,
            origin_kind,
            max_attempts,
        )
        .fetch_all(self.write_pool())
        .await?;
        Ok(ids)
    }
}
