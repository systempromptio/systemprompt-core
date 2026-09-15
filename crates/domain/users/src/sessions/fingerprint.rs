//! Fingerprint session lookups.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SessionRepository;
use crate::Result;
use systemprompt_identifiers::SessionId;

impl SessionRepository {
    pub async fn fingerprint_session_ids(
        &self,
        fingerprint: &str,
        window_days: i64,
    ) -> Result<Vec<SessionId>> {
        let window_days = i32::try_from(window_days).unwrap_or(i32::MAX);
        Ok(sqlx::query_scalar!(
            r#"
            SELECT session_id as "session_id!: SessionId"
            FROM user_sessions
            WHERE fingerprint_hash = $1
              AND started_at > CURRENT_TIMESTAMP - make_interval(days => $2)
            "#,
            fingerprint,
            window_days,
        )
        .fetch_all(&*self.write_pool)
        .await?)
    }
    pub async fn count_active_fingerprint(&self, fingerprint_hash: &str) -> Result<i32> {
        let row = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)::INT as "count!"
            FROM user_sessions
            WHERE fingerprint_hash = $1
              AND ended_at IS NULL
              AND last_activity_at > CURRENT_TIMESTAMP - INTERVAL '7 days'
            "#,
            fingerprint_hash,
        )
        .fetch_one(&*self.write_pool)
        .await?;

        Ok(row)
    }
    pub async fn find_reusable_fingerprint(
        &self,
        fingerprint_hash: &str,
    ) -> Result<Option<SessionId>> {
        let row = sqlx::query_scalar!(
            r#"
            SELECT session_id as "session_id!: SessionId"
            FROM user_sessions
            WHERE fingerprint_hash = $1
              AND ended_at IS NULL
              AND last_activity_at > CURRENT_TIMESTAMP - INTERVAL '7 days'
            ORDER BY last_activity_at ASC
            LIMIT 1
            "#,
            fingerprint_hash,
        )
        .fetch_optional(&*self.write_pool)
        .await?;

        Ok(row)
    }
}
