//! Consumer inventory inputs: where a skill came from, whether it is available,
//! and how a consumer is a member of the managed catalogue.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{InventoryEntryId, ManagedResourceId, ManagedSourceId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InventoryOrigin {
    Configured,
    Managed,
    Imported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InventoryAvailability {
    Available,
    Unavailable,
    Withdrawn,
    Conflicting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InventoryMembership {
    Known {
        effective_from: DateTime<Utc>,
        effective_until: Option<DateTime<Utc>>,
    },
    Unknown,
}
