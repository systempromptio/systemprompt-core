//! [`LeaseRepository`] over `instance_leases`: one owner per key, expiring
//! unless the owner claims again.
//!
//! A claim takes the row when it is absent, expired, already ours, or held by
//! a replica that is no longer alive — the same judgement as
//! [`systemprompt_traits::InstanceRegistry::is_alive`], evaluated inside the
//! claiming statement so two replicas racing for a dead owner's lease cannot
//! both win it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{InstanceId, LeaseKey};
use systemprompt_traits::{
    CoordinationError, CoordinationResult, Lease, LeaseRepository, RepositoryError,
};

use super::service::{INSTANCE_CLAIM_CLASS, INSTANCE_DEAD_AFTER_SECS};
use crate::DbPool;

const MAX_LEASE_TTL: Duration = Duration::from_hours(24);

#[derive(Debug, Clone)]
pub struct InstanceLeaseRepository {
    write_pool: Arc<PgPool>,
    instance_id: InstanceId,
}

struct LeaseRow {
    lease_key: LeaseKey,
    owner_instance_id: String,
    acquired_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl LeaseRow {
    fn into_lease(self) -> Lease {
        Lease {
            key: self.lease_key,
            owner: InstanceId::new(self.owner_instance_id),
            acquired_at: self.acquired_at,
            expires_at: self.expires_at,
        }
    }
}

impl InstanceLeaseRepository {
    pub fn new(db: &DbPool, instance_id: InstanceId) -> Self {
        Self {
            write_pool: db.write_pool(),
            instance_id,
        }
    }

    pub const fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }
}

fn ttl_secs(ttl: Duration) -> CoordinationResult<f64> {
    if ttl < Duration::from_secs(1) || ttl > MAX_LEASE_TTL {
        return Err(CoordinationError::InvalidTtl { ttl });
    }
    Ok(ttl.as_secs_f64())
}

#[async_trait]
impl LeaseRepository for InstanceLeaseRepository {
    async fn claim(&self, key: &LeaseKey, ttl: Duration) -> CoordinationResult<Option<Lease>> {
        let ttl = ttl_secs(ttl)?;
        let row = sqlx::query_as!(
            LeaseRow,
            r#"
            INSERT INTO instance_leases AS held
                (lease_key, owner_instance_id, acquired_at, expires_at)
            VALUES ($1, $2, CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP + make_interval(secs => $3::double precision))
            ON CONFLICT (lease_key) DO UPDATE SET
                owner_instance_id = EXCLUDED.owner_instance_id,
                acquired_at = CASE
                    WHEN held.owner_instance_id = EXCLUDED.owner_instance_id
                        THEN held.acquired_at
                    ELSE EXCLUDED.acquired_at
                END,
                expires_at = EXCLUDED.expires_at
            WHERE held.owner_instance_id = EXCLUDED.owner_instance_id
               OR held.expires_at <= CURRENT_TIMESTAMP
               OR NOT (
                   EXISTS (
                       SELECT 1 FROM pg_locks l
                       WHERE l.locktype = 'advisory'
                         AND l.granted
                         AND l.classid = ($4::int)::oid
                         AND l.objid = (hashtext(held.owner_instance_id))::oid
                         AND l.objsubid = 2
                   )
                   OR EXISTS (
                       SELECT 1 FROM services s
                       WHERE s.instance_id = held.owner_instance_id
                         AND s.heartbeat_at
                             >= CURRENT_TIMESTAMP - make_interval(secs => $5::double precision)
                   )
               )
            RETURNING lease_key AS "lease_key!: LeaseKey", owner_instance_id AS "owner_instance_id!",
                      acquired_at AS "acquired_at!", expires_at AS "expires_at!"
            "#,
            key.as_str(),
            self.instance_id.as_str(),
            ttl,
            INSTANCE_CLAIM_CLASS,
            INSTANCE_DEAD_AFTER_SECS as f64
        )
        .fetch_optional(&*self.write_pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(row.map(LeaseRow::into_lease))
    }

    async fn release(&self, key: &LeaseKey) -> CoordinationResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM instance_leases WHERE lease_key = $1 AND owner_instance_id = $2",
            key.as_str(),
            self.instance_id.as_str()
        )
        .execute(&*self.write_pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(result.rows_affected() > 0)
    }

    async fn holder(&self, key: &LeaseKey) -> CoordinationResult<Option<Lease>> {
        let row = sqlx::query_as!(
            LeaseRow,
            r#"
            SELECT lease_key AS "lease_key: LeaseKey", owner_instance_id, acquired_at, expires_at
            FROM instance_leases
            WHERE lease_key = $1 AND expires_at > CURRENT_TIMESTAMP
            "#,
            key.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(row.map(LeaseRow::into_lease))
    }
}
