//! Installation coverage reads and the per-owner rebuild.
//!
//! The rebuild is the schema's `refresh_managed_installation_coverage(keys)`
//! projector (`managed_installation_coverage_queue.sql`); the
//! `managed_inventory_refresh` job drains the owners the ledger triggers
//! queued, and this entry point rebuilds one owner on demand.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::inventory::InstallationCoverage;
use crate::managed::{ManagedRepository, Result};
use systemprompt_identifiers::{ManagedResourceId, UserId};

impl ManagedRepository {
    pub async fn refresh_installation_coverage(&self, owner: &UserId) -> Result<u64> {
        let changed = sqlx::query_scalar!(
            r#"SELECT refresh_managed_installation_coverage(ARRAY[$1::text]) AS "changed!""#,
            owner.as_str()
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(u64::try_from(changed).unwrap_or(0))
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
