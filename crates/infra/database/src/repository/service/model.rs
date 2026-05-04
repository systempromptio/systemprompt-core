//! Data models for the [`super::ServiceRepository`].

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Row in the `services` registry. Column types mirror the SQL schema; the
/// `created_at` / `updated_at` timestamps are returned as ISO-8601 strings to
/// keep the type plain-serializable.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Globally unique service name.
    pub name: String,
    /// Owning module identifier (e.g. `"agent"`, `"mcp"`).
    pub module_name: String,
    /// Lifecycle status: `pending`, `running`, `stopped`, `error`, …
    pub status: String,
    /// PID of the running process, if any.
    pub pid: Option<i32>,
    /// Allocated TCP port.
    pub port: i32,
    /// Mtime (Unix seconds) of the service binary at registration time.
    pub binary_mtime: Option<i64>,
    /// Row creation timestamp (ISO-8601 text).
    pub created_at: String,
    /// Row last-update timestamp (ISO-8601 text).
    pub updated_at: String,
}

/// Borrowed inputs for [`super::ServiceRepository::create_service`].
#[derive(Debug)]
pub struct CreateServiceInput<'a> {
    /// Globally unique service name.
    pub name: &'a str,
    /// Owning module identifier.
    pub module_name: &'a str,
    /// Initial lifecycle status.
    pub status: &'a str,
    /// Allocated TCP port.
    pub port: u16,
    /// Mtime (Unix seconds) of the service binary, if known.
    pub binary_mtime: Option<i64>,
}
