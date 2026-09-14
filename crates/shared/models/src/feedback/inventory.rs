//! Skill feedback contracts shared across ingestion, marketplace, evaluators
//! and clients.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{InventoryEntryId, ManagedResourceId, ManagedSourceId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryOrigin {
    Configured,
    Managed,
    Imported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryAvailability {
    Available,
    Unavailable,
    Withdrawn,
    Conflicting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryInput {
    pub entry_id: InventoryEntryId,
    pub organization_owner_id: UserId,
    pub origin: InventoryOrigin,
    pub configured_key: Option<String>,
    pub resource_id: Option<ManagedResourceId>,
    pub source_id: Option<ManagedSourceId>,
    pub availability: InventoryAvailability,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InventoryMembership {
    Known {
        effective_from: DateTime<Utc>,
        effective_until: Option<DateTime<Utc>>,
    },
    Unknown,
}
