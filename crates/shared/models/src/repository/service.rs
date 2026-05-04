use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use systemprompt_traits::RepositoryError;

use crate::errors::RowParseError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRecord {
    pub name: String,
    pub module_name: String,
    pub status: String,
    pub pid: Option<i32>,
    pub port: i32,
}

impl ServiceRecord {
    /// Build a [`ServiceRecord`] from a JSON-shaped row map produced by
    /// the runtime SQL adapter.
    ///
    /// # Errors
    ///
    /// Returns [`RowParseError::Missing`] when a required column is
    /// absent or has the wrong type, or [`RowParseError::OutOfRange`]
    /// when the `port` column exceeds the `i32` range.
    pub fn from_json_row(
        row: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Self, RowParseError> {
        let name = row
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or(RowParseError::Missing("name"))?
            .to_string();

        let module_name = row
            .get("module_name")
            .and_then(|v| v.as_str())
            .ok_or(RowParseError::Missing("module_name"))?
            .to_string();

        let status = row
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or(RowParseError::Missing("status"))?
            .to_string();

        let pid = row
            .get("pid")
            .and_then(serde_json::Value::as_i64)
            .and_then(|i| i32::try_from(i).ok());

        let port = row
            .get("port")
            .and_then(serde_json::Value::as_i64)
            .ok_or(RowParseError::Missing("port"))
            .and_then(|i| i32::try_from(i).map_err(|_| RowParseError::OutOfRange("port")))?;

        Ok(Self {
            name,
            module_name,
            status,
            pid,
            port,
        })
    }
}

/// Repository operations for managing the lifecycle of installed services.
///
/// This trait is `dyn`-compatible because the running service relies on
/// trait objects for storage backend selection — `#[async_trait]` is
/// required.
#[async_trait]
pub trait ServiceLifecycle: Send + Sync {
    /// List every service the database currently considers running.
    async fn get_running_services(&self) -> Result<Vec<ServiceRecord>, RepositoryError>;
    /// Mark a service as crashed by name.
    async fn mark_crashed(&self, service_name: &str) -> Result<(), RepositoryError>;
    /// Set the status field of a named service.
    async fn update_status(&self, service_name: &str, status: &str) -> Result<(), RepositoryError>;
}
