//! Per-replica role presence: every replica records its `server.role` with
//! each registry heartbeat, so the live replica count per role is readable
//! cluster-wide, including for roles that register no process, and the live
//! members of one role are listable for spreading work across them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::NodeRole;

use super::liveness::INSTANCE_DEAD_AFTER_SECS;
use super::repo::ServiceRepository;
use crate::error::DatabaseResult;

/// The number of live replicas currently heartbeating one role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleCount {
    pub role: String,
    pub instances: i64,
}

impl ServiceRepository {
    pub async fn touch_presence(&self, role: NodeRole) -> DatabaseResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO instance_presence (instance_id, role, heartbeat_at)
            VALUES ($1, $2, CURRENT_TIMESTAMP)
            ON CONFLICT (instance_id) DO UPDATE SET
                role = EXCLUDED.role,
                heartbeat_at = EXCLUDED.heartbeat_at
            "#,
            self.instance_id.as_str(),
            role.as_str()
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn count_live_instances_by_role(&self) -> DatabaseResult<Vec<RoleCount>> {
        let rows = sqlx::query_as!(
            RoleCount,
            r#"
            SELECT role, COUNT(*) AS "instances!"
            FROM instance_presence
            WHERE heartbeat_at
                  >= CURRENT_TIMESTAMP - make_interval(secs => $1::double precision)
            GROUP BY role
            ORDER BY role
            "#,
            INSTANCE_DEAD_AFTER_SECS as f64
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_reachable_instances(
        &self,
        role: NodeRole,
        fresh_within_secs: i64,
    ) -> DatabaseResult<Vec<InstanceId>> {
        let rows = sqlx::query_scalar!(
            r#"
            SELECT p.instance_id AS "instance_id!: InstanceId"
            FROM instance_presence p
            JOIN instance_endpoints e ON e.instance_id = p.instance_id
            WHERE p.role = $1
              AND p.heartbeat_at
                  >= CURRENT_TIMESTAMP - make_interval(secs => $2::double precision)
              AND e.heartbeat_at
                  >= CURRENT_TIMESTAMP - make_interval(secs => $2::double precision)
            ORDER BY p.instance_id
            "#,
            role.as_str(),
            fresh_within_secs as f64
        )
        .fetch_all(&*self.write_pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete_stale_presence(&self, older_than_secs: i64) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM instance_presence
            WHERE heartbeat_at < CURRENT_TIMESTAMP - make_interval(secs => $1::double precision)
            "#,
            older_than_secs as f64
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }
}
