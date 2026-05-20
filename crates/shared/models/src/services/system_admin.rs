//! System-admin identity: the explicit, validated owner row that the
//! platform attributes system-initiated work to (scheduler bootstrap jobs,
//! gateway telemetry, default MCP server owners).
//!
//! Resolution is a one-shot operation performed during runtime bootstrap:
//! the profile-supplied [`SystemAdminConfig`] is looked up against the
//! `users` table, validated (active, has `admin` role), and the resulting
//! [`SystemAdmin`] is parked in a process-wide `OnceLock` so that
//! attribution sites without an `AppContext` handle (logging hot path,
//! MCP registry inventory) read it through [`SystemAdmin::current`]
//! instead of fabricating a sentinel.

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

    /// Park the resolved admin in the process-wide cell or return the
    /// handle already installed by a prior bootstrap. The platform
    /// resolves the admin once per process, but a CLI process may build
    /// several `AppContext`s in sequence; the second build observes the
    /// installed handle and reuses it instead of failing.
    pub fn get_or_install(value: Self) -> &'static Self {
        SYSTEM_ADMIN.get_or_init(|| value)
    }

    pub fn current() -> Result<&'static Self, SystemAdminNotInitialized> {
        SYSTEM_ADMIN.get().ok_or(SystemAdminNotInitialized)
    }

    pub fn current_id() -> Result<&'static UserId, SystemAdminNotInitialized> {
        Self::current().map(Self::id)
    }
}

static SYSTEM_ADMIN: OnceLock<SystemAdmin> = OnceLock::new();

#[derive(Debug, Clone, Copy, Error)]
#[error(
    "system admin not resolved: AppContext bootstrap must run before any system-attributed work"
)]
pub struct SystemAdminNotInitialized;
