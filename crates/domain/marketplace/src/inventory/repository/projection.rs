//! Complete inventory reconciliation persisted in one transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::inventory::catalog::invalid;
use crate::inventory::projection::{configured_entries, merge_resource, next_generation};
use crate::inventory::{ConfiguredInventoryEntry, InventoryStatus};
use crate::managed::{ManagedError, ManagedRepository, Result};
use systemprompt_identifiers::UserId;

impl ManagedRepository {
    pub async fn reconcile_inventory(
        &self,
        owner: &UserId,
        configured: &[ConfiguredInventoryEntry],
    ) -> Result<InventoryStatus> {
        if configured.len() > 10_000 {
            return Err(invalid("Inventory exceeds 10000 configured entries"));
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "INSERT INTO managed_inventory_state(owner_id) VALUES($1) ON CONFLICT DO NOTHING",
            owner.as_str()
        )
        .execute(&mut *tx)
        .await?;
        let state = sqlx::query!(
            "SELECT generation FROM managed_inventory_state WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;
        let observed = chrono::Utc::now();
        let mut entries = configured_entries(owner, configured)?;
        for resource in self.inventory_resources(&mut tx, owner).await? {
            merge_resource(owner, &mut entries, resource);
        }
        let previous = Self::retire_missing_inventory(&mut tx, owner, &mut entries).await?;
        let generation = next_generation(state.generation, &entries, &previous)?;
        for (id, entry) in &entries {
            let stored = super::StoredInventoryEntry {
                entry,
                previous: previous.get(id),
                generation,
                observed,
            };
            Self::store_inventory_entry(&mut tx, owner, &stored).await?;
        }
        let count = i64::try_from(entries.len()).map_err(|error| ManagedError::Internal {
            context: "inventory count overflow",
            source: Box::new(error),
        })?;
        let sources = serde_json::to_value(systemprompt_loader::bundle::sources_provenance())?;
        sqlx::query!("UPDATE managed_inventory_state SET generation=$2,observed_at=$3,entries=$4,sources=$5,last_error=NULL WHERE owner_id=$1",owner.as_str(),generation,observed,count,sources).execute(&mut *tx).await?;
        // Why: membership rows join on this generation's mint time as
        // `effective_from`; an unchanged pass must not move it.
        sqlx::query!("INSERT INTO managed_inventory_observations(owner_id,generation,observed_at,entries,sources) VALUES($1,$2,$3,$4,$5) ON CONFLICT(owner_id,generation) DO NOTHING",owner.as_str(),generation,observed,count,sources).execute(&mut *tx).await?;
        let result = InventoryStatus {
            generation,
            observed_at: Some(observed),
            entries: count,
            last_error: None,
        };
        tx.commit().await?;
        Ok(result)
    }
}
