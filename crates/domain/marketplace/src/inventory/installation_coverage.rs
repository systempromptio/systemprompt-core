//! Installation acknowledgment coverage is cached separately from invocation
//! attribution.
//!
//! The cache is recomputed by the `managed_inventory_refresh` job, which reads
//! back how many resources' coverage changed. A pass that
//! finds every resource's coverage unchanged writes nothing: the generation
//! and `observed_at` on `managed_installation_coverage_state` move only when
//! at least one resource's body changed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::managed::{ManagedRepository, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ManagedResourceId, UserId};

/// Current eligible enrolled-device coverage and retained authenticated
/// installation evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct InstallationCoverage {
    pub eligible_devices: i64,
    pub current_acknowledged_devices: i64,
    pub current_verified_devices: i64,
    pub acknowledged_installations: i64,
    pub unverifiable_installations: i64,
    pub legacy_receipts: i64,
}

impl ManagedRepository {
    pub async fn refresh_installation_coverage(&self, owner: &UserId) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!("INSERT INTO managed_installation_coverage_state(owner_id) VALUES($1) ON CONFLICT DO NOTHING",owner.as_str()).execute(&mut *tx).await?;
        let previous=sqlx::query_scalar!("SELECT generation FROM managed_installation_coverage_state WHERE owner_id=$1 FOR UPDATE",owner.as_str()).fetch_one(&mut *tx).await?;
        let generation = previous
            .checked_add(1)
            .ok_or_else(|| super::catalog::invalid("Installation coverage generation overflow"))?;
        let changed=sqlx::query!(r#"INSERT INTO managed_installation_coverage(owner_id,resource_id,body,generation)
   WITH latest AS(SELECT DISTINCT ON(resource_id,consumer_id,device_id,host,installation_id) * FROM managed_installation_receipts WHERE owner_id=$1 AND consumer_id IS NOT NULL ORDER BY resource_id,consumer_id,device_id,host,installation_id,generation DESC,verified_at DESC),
   eligible AS(SELECT g.resource_id,d.device_id,g.consumer_id FROM managed_consumer_grants g CROSS JOIN LATERAL public.active_devices_for_consumer(g.consumer_id) d WHERE g.owner_id=$1 AND g.revoked_at IS NULL),
   current_receipts AS(SELECT l.* FROM latest l JOIN managed_publication_selections p ON p.owner_id=l.owner_id AND p.resource_id=l.resource_id AND p.generation=l.generation AND p.state='published' JOIN eligible e ON e.resource_id=l.resource_id AND e.device_id=l.device_id AND e.consumer_id=l.consumer_id)
   SELECT $1,r.id,jsonb_build_object(
    'eligible_devices',(SELECT COUNT(DISTINCT device_id) FROM eligible e WHERE e.resource_id=r.id),
    'current_acknowledged_devices',(SELECT COUNT(DISTINCT device_id) FROM current_receipts c WHERE c.resource_id=r.id),
    'current_verified_devices',(SELECT COUNT(DISTINCT device_id) FROM current_receipts c WHERE c.resource_id=r.id AND fully_verified),
    'acknowledged_installations',(SELECT COUNT(*) FROM latest l WHERE l.resource_id=r.id),
    'unverifiable_installations',(SELECT COUNT(*) FROM latest l WHERE l.resource_id=r.id AND NOT COALESCE(fully_verified,false)),
    'legacy_receipts',(SELECT COUNT(*) FROM managed_installation_receipts l WHERE l.owner_id=$1 AND l.resource_id=r.id AND l.consumer_id IS NULL)), $2
   FROM managed_resources r WHERE r.owner_id=$1
   ON CONFLICT(owner_id,resource_id) DO UPDATE SET body=EXCLUDED.body,generation=EXCLUDED.generation WHERE managed_installation_coverage.body IS DISTINCT FROM EXCLUDED.body"#,owner.as_str(),generation).execute(&mut *tx).await?.rows_affected();
        if changed > 0 {
            sqlx::query!("UPDATE managed_installation_coverage_state SET generation=$2,observed_at=$3 WHERE owner_id=$1",owner.as_str(),generation,Utc::now()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
    pub async fn installation_coverage(
        &self,
        owner: &UserId,
        resource: &ManagedResourceId,
    ) -> Result<Option<InstallationCoverage>> {
        let row = sqlx::query_scalar!(
            "SELECT body FROM managed_installation_coverage WHERE owner_id=$1 AND resource_id=$2",
            owner.as_str(),
            resource.as_str()
        )
        .fetch_optional(&self.pool)
        .await?;
        row.map(serde_json::from_value)
            .transpose()
            .map_err(Into::into)
    }
}
