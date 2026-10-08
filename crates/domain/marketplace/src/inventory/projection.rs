//! Complete inventory reconciliation preserves managed content and publication
//! selection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::catalog::invalid;
use super::types::managed_identity;
use super::{ConfiguredInventoryEntry, InventoryEntry, configured_identity};
use crate::managed::Result;
use std::collections::BTreeMap;
use systemprompt_identifiers::{InventoryEntryId, UserId};
use systemprompt_models::feedback::inventory::{
    InventoryAvailability as Availability, InventoryOrigin as Origin,
};

// Why: a generation is what a campaign, a publication and a diff are pinned to,
// so it may only move when the observed inventory moved. Retirement puts every
// vanished entry back into `entries` carrying a changed record, so comparing
// records covers set changes as well as edits.
pub(super) fn next_generation(
    current: i64,
    records: &BTreeMap<InventoryEntryId, InventoryEntry>,
    previous: &BTreeMap<InventoryEntryId, InventoryEntry>,
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

pub(super) fn configured_entries(
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

pub(super) fn merge_resource(
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
