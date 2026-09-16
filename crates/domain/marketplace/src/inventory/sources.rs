//! Repository-side inventory source projection and retirement of absent
//! identities.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::InventoryEntry;
use super::catalog::invalid;
use super::types::InventoryResource;
use crate::managed::{ManagedRepository, Result};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use systemprompt_identifiers::{
    InventoryEntryId, ManagedResourceId, ManagedSourceId, ResourceRevisionId, UserId,
};
use systemprompt_models::feedback::inventory::InventoryAvailability as Availability;

impl ManagedRepository {
    pub(super) async fn inventory_resources(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
    ) -> Result<Vec<InventoryResource>> {
        let resources=sqlx::query!(r#"SELECT r.id,r.source_id,r.kind,r.resource_key,s.kind AS source_kind,p.state AS "publication_state?",p.revision_id AS published_revision,
            (SELECT v.id FROM managed_revisions v WHERE v.owner_id=r.owner_id AND v.resource_id=r.id ORDER BY v.created_at DESC,v.id DESC LIMIT 1) AS "latest_revision?",
            b.entry_id AS "bound_entry?",e.configured_key AS bound_path,
            EXISTS(SELECT 1 FROM managed_reconciliations x WHERE x.owner_id=r.owner_id AND x.resource_id=r.id AND x.status='open') AS "open_reconciliation!",
            EXISTS(SELECT 1 FROM managed_withdrawal_proposals w WHERE w.owner_id=r.owner_id AND w.resource_id=r.id AND w.status='pending') AS "upstream_removed!"
            FROM managed_resources r JOIN managed_sources s ON s.owner_id=r.owner_id AND s.id=r.source_id
            LEFT JOIN managed_publication_selections p ON p.owner_id=r.owner_id AND p.resource_id=r.id
            LEFT JOIN managed_inventory_bindings b ON b.owner_id=r.owner_id AND b.resource_id=r.id
            LEFT JOIN managed_inventory_entries e ON e.owner_id=b.owner_id AND e.entry_id=b.entry_id
            WHERE r.owner_id=$1 ORDER BY r.id LIMIT 10001"#,owner.as_str()).fetch_all(&mut **tx).await?;
        if resources.len() > 10_000 {
            return Err(invalid("Inventory exceeds 10000 managed entries"));
        }
        Ok(resources
            .into_iter()
            .map(|row| InventoryResource {
                id: ManagedResourceId::new(row.id),
                source_id: ManagedSourceId::new(row.source_id),
                kind: row.kind,
                resource_key: row.resource_key,
                source_kind: row.source_kind,
                publication_state: row.publication_state,
                published_revision: row.published_revision.map(ResourceRevisionId::new),
                latest_revision: row.latest_revision.map(ResourceRevisionId::new),
                bound_entry: row.bound_entry.map(InventoryEntryId::new),
                bound_path: row.bound_path,
                upstream_removed: row.upstream_removed,
                open_reconciliation: row.open_reconciliation,
            })
            .collect())
    }
    pub(super) async fn retire_missing_inventory(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        entries: &mut BTreeMap<InventoryEntryId, InventoryEntry>,
    ) -> Result<BTreeMap<InventoryEntryId, serde_json::Value>> {
        let rows=sqlx::query!("SELECT entry_id,record FROM managed_inventory_membership WHERE owner_id=$1 AND effective_until IS NULL",owner.as_str()).fetch_all(&mut **tx).await?;
        let mut previous = BTreeMap::new();
        for row in rows {
            let id = InventoryEntryId::new(row.entry_id);
            if let Entry::Vacant(slot) = entries.entry(id.clone()) {
                let mut entry: InventoryEntry = serde_json::from_value(row.record.clone())?;
                entry.availability = Availability::Withdrawn;
                entry.diagnostic = Some(
                    "Entry is absent from the latest complete inventory observation".to_owned(),
                );
                slot.insert(entry);
            }
            previous.insert(id, row.record);
        }
        Ok(previous)
    }
}
