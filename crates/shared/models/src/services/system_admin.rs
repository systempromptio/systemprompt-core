//! System-admin identity: the explicit, validated owner row that the
//! platform attributes system-initiated work to (scheduler bootstrap jobs,
//! gateway telemetry, default MCP server owners).
//!
//! Resolution is a one-shot operation performed during runtime bootstrap:
//! the profile-supplied [`SystemAdminConfig`] is looked up against the
//! `users` table, validated (active, has `admin` role), and the resulting
//! [`SystemAdmin`] is handed to `AppContext`. From there, every consumer
//! that needs the platform owner takes it as a constructor argument; the
//! only exception is logging attribution, which parks the value in a
//! cell scoped to `systemprompt_logging`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

/// Profile-supplied configuration for the platform owner. Must resolve at
/// startup to an active user row carrying the `admin` role; the platform
/// refuses to boot otherwise.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SystemAdminConfig {
    pub username: String,
}

/// Resolved system-admin handle threaded through `AppContext`. Holds the
/// typed `UserId` of the actual `users` row, not a sentinel.
#[derive(Debug, Clone)]
pub struct SystemAdmin {
    id: UserId,
    username: String,
}

impl SystemAdmin {
    #[must_use]
    pub const fn new(id: UserId, username: String) -> Self {
        Self { id, username }
    }

    #[must_use]
    pub const fn id(&self) -> &UserId {
        &self.id
    }

    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}
