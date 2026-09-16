//! Owner-scoped inventory persistence, explicit bindings and effective-time
//! reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::catalog::invalid;
use super::{InventoryEntry, InventoryStatus};
use crate::managed::{ManagedError, ManagedRepository, Result};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    InventoryEntryId, ManagedResourceId, ManagedSourceId, ResourceRevisionId, UserId,
};

impl ManagedRepository {
    pub(super) async fn store_inventory_entry(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        entry: &InventoryEntry,
        generation: i64,
        observed: DateTime<Utc>,
    ) -> Result<()> {
        let origin = serde_json::to_value(entry.origin)?
            .as_str()
            .ok_or_else(|| invalid("Invalid inventory origin"))?
            .to_owned();
        let availability = serde_json::to_value(entry.availability)?
            .as_str()
            .ok_or_else(|| invalid("Invalid inventory availability"))?
            .to_owned();
        let record = serde_json::to_value(entry)?;
        let previous=sqlx::query_scalar!("SELECT record FROM managed_inventory_membership WHERE owner_id=$1 AND entry_id=$2 AND effective_until IS NULL",owner.as_str(),entry.entry_id.as_str()).fetch_optional(&mut **tx).await?;
        sqlx::query!("INSERT INTO managed_inventory_entries(owner_id,entry_id,kind,resource_key,origin,configured_key,resource_id,source_id,availability,latest_revision_id,published_revision_id,diagnostic,first_observed_at,last_observed_at,generation) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$13,$14) ON CONFLICT(owner_id,entry_id) DO UPDATE SET kind=EXCLUDED.kind,resource_key=EXCLUDED.resource_key,origin=EXCLUDED.origin,configured_key=EXCLUDED.configured_key,resource_id=EXCLUDED.resource_id,source_id=EXCLUDED.source_id,availability=EXCLUDED.availability,latest_revision_id=EXCLUDED.latest_revision_id,published_revision_id=EXCLUDED.published_revision_id,diagnostic=EXCLUDED.diagnostic,last_observed_at=EXCLUDED.last_observed_at,generation=EXCLUDED.generation",owner.as_str(),entry.entry_id.as_str(),&entry.kind,&entry.resource_key,origin,entry.configured_key.as_deref(),entry.resource_id.as_ref().map(ManagedResourceId::as_str),entry.source_id.as_ref().map(ManagedSourceId::as_str),availability,entry.latest_revision_id.as_ref().map(ResourceRevisionId::as_str),entry.published_revision_id.as_ref().map(ResourceRevisionId::as_str),entry.diagnostic.as_deref(),observed,generation).execute(&mut **tx).await?;
        if previous.as_ref() != Some(&record) {
            sqlx::query!("UPDATE managed_inventory_membership SET effective_until=$3 WHERE owner_id=$1 AND entry_id=$2 AND effective_until IS NULL",owner.as_str(),entry.entry_id.as_str(),observed).execute(&mut **tx).await?;
            sqlx::query!("INSERT INTO managed_inventory_membership(owner_id,entry_id,effective_from,record) VALUES($1,$2,$3,$4)",owner.as_str(),entry.entry_id.as_str(),observed,record).execute(&mut **tx).await?;
        }
        Ok(())
    }

    pub async fn inventory_status(&self, owner: &UserId) -> Result<InventoryStatus> {
        let row=sqlx::query!("SELECT generation,observed_at,entries,last_error FROM managed_inventory_state WHERE owner_id=$1",owner.as_str()).fetch_optional(&self.pool).await?;
        Ok(row.map_or(
            InventoryStatus {
                generation: 0,
                observed_at: None,
                entries: 0,
                last_error: None,
            },
            |row| InventoryStatus {
                generation: row.generation,
                observed_at: row.observed_at,
                entries: row.entries,
                last_error: row.last_error,
            },
        ))
    }

    pub async fn record_inventory_failure(&self, owner: &UserId) -> Result<()> {
        sqlx::query!("INSERT INTO managed_inventory_state(owner_id,last_error) VALUES($1,'Inventory scan failed; previous membership retained, inspect configured catalog availability') ON CONFLICT(owner_id) DO UPDATE SET last_error=EXCLUDED.last_error",owner.as_str()).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn inventory(
        &self,
        owner: &UserId,
        after: Option<&InventoryEntryId>,
        limit: u32,
    ) -> Result<Vec<InventoryEntry>> {
        if !(1..=100).contains(&limit) {
            return Err(invalid("Inventory page size must be 1–100"));
        }
        let after = after.map(InventoryEntryId::as_str);
        let limit = i64::from(limit);
        let rows=sqlx::query_scalar!("SELECT record FROM managed_inventory_membership WHERE owner_id=$1 AND effective_until IS NULL AND ($2::text IS NULL OR entry_id>$2) ORDER BY entry_id LIMIT $3",owner.as_str(),after,limit).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| serde_json::from_value(row).map_err(Into::into))
            .collect()
    }

    pub async fn inventory_entry(
        &self,
        owner: &UserId,
        id: &InventoryEntryId,
    ) -> Result<InventoryEntry> {
        let record=sqlx::query_scalar!("SELECT record FROM managed_inventory_membership WHERE owner_id=$1 AND entry_id=$2 AND effective_until IS NULL",owner.as_str(),id.as_str()).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        Ok(serde_json::from_value(record)?)
    }
}
