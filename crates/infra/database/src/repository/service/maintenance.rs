//! Liveness maintenance on the `services` registry: heartbeats, stale-row
//! cleanup for this instance, and the cross-instance reap of dead replicas.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::repo::ServiceRepository;
use crate::error::DatabaseResult;

impl ServiceRepository {
    pub async fn cleanup_stale_entries(&self) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM services
            WHERE instance_id = $1
              AND (status IN ('error', 'crashed')
                   OR (status = 'running' AND pid IS NULL))
            "#,
            self.instance_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn touch_heartbeat(&self) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            r#"UPDATE services SET heartbeat_at = CURRENT_TIMESTAMP WHERE instance_id = $1"#,
            self.instance_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_dead_instances(&self, older_than_secs: i64) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM services
            WHERE heartbeat_at < CURRENT_TIMESTAMP - make_interval(secs => $1::double precision)
            "#,
            older_than_secs as f64
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }
}
