//! System-admin identity: the explicit, validated owner row that the
//! platform attributes system-initiated work to (scheduler bootstrap jobs,
//! gateway telemetry, default MCP server owners).
//!
//! Resolution is a one-shot operation performed during runtime bootstrap:
//! the profile-supplied [`SystemAdminConfig`] is looked up against the
//! `users` table, validated (active, has `admin` role), and the resulting
//! [`SystemAdmin`] is installed into a process-wide `OnceLock`. Every
//! downstream consumer reads through that handle instead of inventing a
//! `UserId::admin()` string.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;
use thiserror::Error;

/// Profile-supplied configuration for the platform owner. Must resolve at
/// startup to an active user row carrying the `admin` role; the platform
/// refuses to boot otherwise.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SystemAdminConfig {
    pub username: String,
}

/// Resolved system-admin handle parked on `AppContext` and in the
/// process-wide cell. Holds the typed `UserId` of the actual `users` row,
/// not a sentinel.
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

    /// Install the process-wide system-admin handle. Fails if a value was
    /// already installed — the runtime resolves the admin exactly once.
    pub fn install(value: Self) -> Result<(), Box<Self>> {
        SYSTEM_ADMIN.set(value).map_err(Box::new)
    }

    pub fn current() -> Result<&'static Self, SystemAdminNotInitialized> {
        SYSTEM_ADMIN.get().ok_or(SystemAdminNotInitialized)
    }

    pub fn current_id() -> Result<&'static UserId, SystemAdminNotInitialized> {
        Self::current().map(Self::id)
    }

    #[must_use]
    pub fn is_initialized() -> bool {
        SYSTEM_ADMIN.get().is_some()
    }
}

static SYSTEM_ADMIN: OnceLock<SystemAdmin> = OnceLock::new();

#[derive(Debug, Clone, Copy, Error)]
#[error(
    "system admin not initialized: SystemAdmin::install must be called during runtime bootstrap \
     before any system-attributed work is performed"
)]
pub struct SystemAdminNotInitialized;
