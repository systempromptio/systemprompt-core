//! Advertised replica addresses and the lookup that finds which live replica
//! owns a named MCP or agent process.
//!
//! A replica that supervises processes records the address its peers reach it
//! on with every registry heartbeat; a replica without a local process
//! forwards to the owner [`ServiceRepository::find_process_owner`] names. The
//! owner must be heartbeating both its endpoint and its `services` row within
//! [`super::INSTANCE_DEAD_AFTER_SECS`], so a crashed owner is never chosen.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_manifest::services::ServiceStatus;

use super::liveness::INSTANCE_DEAD_AFTER_SECS;
use super::repo::ServiceRepository;
use crate::error::DatabaseResult;

/// The live replica running a named process and the address it advertises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOwner {
    pub instance_id: InstanceId,
    pub advertise_url: String,
}

impl ServiceRepository {
    pub async fn advertise_endpoint(&self, advertise_url: &str) -> DatabaseResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO instance_endpoints (instance_id, advertise_url, heartbeat_at)
            VALUES ($1, $2, CURRENT_TIMESTAMP)
            ON CONFLICT (instance_id) DO UPDATE SET
                advertise_url = EXCLUDED.advertise_url,
                heartbeat_at = EXCLUDED.heartbeat_at
            "#,
            self.instance_id.as_str(),
            advertise_url
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn find_process_owner(
        &self,
        service_name: &ServiceName,
    ) -> DatabaseResult<Option<ProcessOwner>> {
        let row = sqlx::query!(
            r#"
            SELECT s.instance_id AS "instance_id: InstanceId", e.advertise_url
            FROM services s
            JOIN instance_endpoints e ON e.instance_id = s.instance_id
            WHERE s.name = $1
              AND s.status = $2
              AND s.instance_id <> $3
              AND s.heartbeat_at
                  >= CURRENT_TIMESTAMP - make_interval(secs => $4::double precision)
              AND e.heartbeat_at
                  >= CURRENT_TIMESTAMP - make_interval(secs => $4::double precision)
            ORDER BY s.heartbeat_at DESC
            LIMIT 1
            "#,
            service_name.as_str(),
            ServiceStatus::Running.as_str(),
            self.instance_id.as_str(),
            INSTANCE_DEAD_AFTER_SECS as f64
        )
        .fetch_optional(&*self.write_pool)
        .await?;
        Ok(row.map(|row| ProcessOwner {
            instance_id: row.instance_id,
            advertise_url: row.advertise_url,
        }))
    }

    pub async fn delete_stale_endpoints(&self, older_than_secs: i64) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM instance_endpoints
            WHERE heartbeat_at < CURRENT_TIMESTAMP - make_interval(secs => $1::double precision)
            "#,
            older_than_secs as f64
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }
}
