//! Replica liveness for [`InstanceRegistry`]: a replica is alive while its
//! boot-time instance claim is held or its `services` heartbeat is fresher
//! than [`INSTANCE_DEAD_AFTER_SECS`].
//!
//! The claim is the authoritative signal — it vanishes with the claiming
//! session within [`super::CLAIM_DEAD_PEER_SECS`] of a crash — and the
//! heartbeat covers a replica that never claimed (a CLI one-shot) or whose
//! claim was taken on another server. Both are read on the primary: a read
//! replica sees neither the primary's locks nor its latest heartbeat.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::{CoordinationResult, InstanceRegistry, RepositoryError};

use super::claim::INSTANCE_CLAIM_CLASS;
use super::repo::ServiceRepository;

// Why: six missed 15 s heartbeats — long enough that a replica stalled by a
// slow deploy or a GC pause is not judged dead while it is still serving.
pub const INSTANCE_DEAD_AFTER_SECS: i64 = 90;

#[async_trait]
impl InstanceRegistry for ServiceRepository {
    async fn is_alive(&self, instance: &InstanceId) -> CoordinationResult<bool> {
        let alive = sqlx::query_scalar!(
            r#"
            SELECT (
                EXISTS (
                    SELECT 1 FROM pg_locks l
                    WHERE l.locktype = 'advisory'
                      AND l.granted
                      AND l.classid = ($1::int)::oid
                      AND l.objid = (hashtext($2))::oid
                      AND l.objsubid = 2
                )
                OR EXISTS (
                    SELECT 1 FROM services s
                    WHERE s.instance_id = $2
                      AND s.heartbeat_at
                          >= CURRENT_TIMESTAMP - make_interval(secs => $3::double precision)
                )
            ) AS "alive!"
            "#,
            INSTANCE_CLAIM_CLASS,
            instance.as_str(),
            INSTANCE_DEAD_AFTER_SECS as f64
        )
        .fetch_one(&*self.write_pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(alive)
    }
}
