//! Complete inventory reconciliation preserves managed content and publication
//! selection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::catalog::invalid;
use super::types::managed_identity;
use super::{ConfiguredInventoryEntry, InventoryEntry, InventoryStatus, configured_identity};
use crate::managed::{ManagedError, ManagedRepository, Result};
use std::collections::BTreeMap;
use systemprompt_identifiers::{InventoryEntryId, UserId};
use systemprompt_models::feedback::inventory::{
    InventoryAvailability as Availability, InventoryOrigin as Origin,
};

impl ManagedRepository {
    pub async fn reconcile_inventory(
        &self,
        owner: &UserId,
        configured: &[ConfiguredInventoryEntry],
    ) -> Result<InventoryStatus> {
        self.reconcile_inventory_operation(owner, configured, None)
            .await
    }

    pub async fn reconcile_inventory_operation(
        &self,
        owner: &UserId,
        configured: &[ConfiguredInventoryEntry],
        operation: Option<&crate::managed::operations::ApiOperation>,
    ) -> Result<InventoryStatus> {
        if configured.len() > 10_000 {
            return Err(invalid("Inventory exceeds 10000 configured entries"));
        }
        let mut tx = self.pool.begin().await?;
        if let Some(operation) = operation
            && let Some(completed) = Self::inventory_lease(&mut tx, owner, operation).await?
        {
            return Ok(completed);
        }

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
        let records = inventory_records(&entries)?;
        let generation = next_generation(state.generation, &records, &previous)?;
        for (id, entry) in &entries {
            let record = records
                .get(id)
                .ok_or_else(|| invalid("Inventory record lost"))?;
            let stored = super::repository::StoredInventoryEntry {
                entry,
                record,
                previous: previous.get(id),
                generation,
                observed,
            };
            Self::store_inventory_entry(&mut tx, owner, &stored).await?;
        }
        let count = i64::try_from(entries.len())
            .map_err(|error| ManagedError::Invalid(format!("Inventory count overflow: {error}")))?;
        let sources = serde_json::to_value(systemprompt_loader::bundle::sources_provenance())?;
        sqlx::query!("UPDATE managed_inventory_state SET generation=$2,observed_at=$3,entries=$4,sources=$5,last_error=NULL WHERE owner_id=$1",owner.as_str(),generation,observed,count,sources).execute(&mut *tx).await?;
        // Why: an observation row is the record of when its generation was minted,
        // and the membership rows that pass wrote carry exactly that timestamp as
        // `effective_from`. An unchanged pass must not move it — the state row
        // carries "last checked" instead, and moving this one would break the join
        // that reads what each generation added, removed and changed.
        sqlx::query!("INSERT INTO managed_inventory_observations(owner_id,generation,observed_at,entries,sources) VALUES($1,$2,$3,$4,$5) ON CONFLICT(owner_id,generation) DO NOTHING",owner.as_str(),generation,observed,count,sources).execute(&mut *tx).await?;
        let result = InventoryStatus {
            generation,
            observed_at: Some(observed),
            entries: count,
            last_error: None,
        };
        if let Some(operation) = operation {
            sqlx::query!("UPDATE managed_api_operations SET state='completed',result=$3,input_checkpoint=NULL,updated_at=clock_timestamp() WHERE owner_id=$1 AND id=$2",owner.as_str(),operation.id.as_str(),sqlx::types::Json(&result) as _).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(result)
    }
}

impl ManagedRepository {
    async fn inventory_lease(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        operation: &crate::managed::operations::ApiOperation,
    ) -> Result<Option<InventoryStatus>> {
        let current=sqlx::query!("SELECT fence,state,result FROM managed_api_operations WHERE owner_id=$1 AND id=$2 FOR UPDATE",owner.as_str(),operation.id.as_str()).fetch_one(&mut **tx).await?;
        if current.state == "completed" {
            return Ok(Some(serde_json::from_value(
                current
                    .result
                    .ok_or_else(|| invalid("Operation result unavailable"))?,
            )?));
        }
        if current.fence != operation.fence || current.state != "pending" {
            return Err(invalid("Operation lease was superseded"));
        }
        Ok(None)
    }
}

// Why: a generation is what a campaign, a publication and a diff are pinned to,
// so it may only move when the observed inventory moved. Retirement puts every
// vanished entry back into `entries` carrying a changed record, so comparing
// records covers set changes as well as edits.
fn next_generation(
    current: i64,
    records: &BTreeMap<InventoryEntryId, serde_json::Value>,
    previous: &BTreeMap<InventoryEntryId, serde_json::Value>,
) -> Result<i64> {
    if records
        .iter()
        .all(|(id, record)| previous.get(id) == Some(record))
    {
        return Ok(current);
    }
    current
        .checked_add(1)
        .ok_or_else(|| invalid("Inventory generation overflow"))
}

fn inventory_records(
    entries: &BTreeMap<InventoryEntryId, InventoryEntry>,
) -> Result<BTreeMap<InventoryEntryId, serde_json::Value>> {
    entries
        .iter()
        .map(|(id, entry)| Ok((id.clone(), serde_json::to_value(entry)?)))
        .collect()
}

fn configured_entries(
    owner: &UserId,
    configured: &[ConfiguredInventoryEntry],
) -> Result<BTreeMap<InventoryEntryId, InventoryEntry>> {
    let mut entries: BTreeMap<InventoryEntryId, InventoryEntry> = BTreeMap::new();
    for input in configured {
        crate::managed::validate_inventory_path(&input.relative_root)?;
        if input.kind.is_empty() || input.resource_key.is_empty() || input.resource_key.len() > 180
        {
            return Err(invalid("Invalid inventory identity"));
        }
        let id = configured_identity(owner, &input.kind, &input.resource_key);
        let entry = InventoryEntry {
            entry_id: id.clone(),
            kind: input.kind.clone(),
            resource_key: input.resource_key.clone(),
            origin: Origin::Configured,
            configured_key: Some(input.relative_root.clone()),
            resource_id: None,
            source_id: None,
            availability: input.availability,
            latest_revision_id: None,
            published_revision_id: None,
            diagnostic: input.diagnostic.clone(),
        };
        if let Some(previous) = entries.insert(id, entry) {
            let current = entries
                .get_mut(&previous.entry_id)
                .ok_or_else(|| invalid("Inventory collision"))?;
            current.availability = Availability::Conflicting;
            current.diagnostic =
                Some("Multiple configured paths declare the same identity".to_owned());
        }
    }
    Ok(entries)
}

fn merge_resource(
    owner: &UserId,
    entries: &mut BTreeMap<InventoryEntryId, InventoryEntry>,
    resource: super::types::InventoryResource,
) {
    let resource_id = resource.id;
    let canonical = resource
        .bound_entry
        .unwrap_or_else(|| managed_identity(&resource_id));
    let mut entry = entries
        .remove(&canonical)
        .unwrap_or_else(|| InventoryEntry {
            entry_id: canonical.clone(),
            kind: resource.kind.clone(),
            resource_key: resource.resource_key.clone(),
            origin: if resource.source_kind == "managed" {
                Origin::Managed
            } else {
                Origin::Imported
            },
            configured_key: resource.bound_path.clone(),
            resource_id: None,
            source_id: None,
            availability: Availability::Available,
            latest_revision_id: None,
            published_revision_id: None,
            diagnostic: None,
        });
    if resource.bound_path.is_some() && entry.origin != Origin::Configured {
        entry.origin = Origin::Configured;
        entry.availability = Availability::Withdrawn;
        entry.diagnostic = Some(
            "Configured source entry was removed; retained publication is unchanged".to_owned(),
        );
    }
    entry.resource_id = Some(resource_id);
    entry.source_id = Some(resource.source_id);
    entry.latest_revision_id = resource.latest_revision;
    entry.published_revision_id = resource.published_revision;
    if resource.publication_state.as_deref() == Some("withdrawn") {
        entry.availability = Availability::Withdrawn;
    } else if resource.upstream_removed || entry.latest_revision_id.is_none() {
        entry.availability = Availability::Unavailable;
        entry.diagnostic = Some(
            if resource.upstream_removed {
                "Upstream resource was removed; withdrawal requires review"
            } else {
                "No retained revision is available"
            }
            .to_owned(),
        );
    }
    if resource.open_reconciliation {
        entry.availability = Availability::Conflicting;
        entry.diagnostic =
            Some("Incoming source changes require three-way reconciliation".to_owned());
    }
    let configured_id = configured_identity(owner, &entry.kind, &entry.resource_key);
    if canonical != configured_id
        && let Some(conflicting) = entries.get_mut(&configured_id)
    {
        conflicting.availability = Availability::Conflicting;
        conflicting.diagnostic =
            Some("A managed resource shares this name; explicit binding is required".to_owned());
        entry.availability = Availability::Conflicting;
        entry.diagnostic.clone_from(&conflicting.diagnostic);
    }
    entries.insert(canonical, entry);
}
