//! Managed-service record and lifecycle trait.
//!
//! [`ServiceLifecycle`] is invoked through trait objects, so it uses
//! `#[async_trait]` — native `async fn` in traits is not yet
//! `dyn`-compatible.

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
    pub fn from_json_row(
        row: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Self, RowParseError> {
        let name = row
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or(RowParseError::Missing("name"))?
            .to_owned();

        let module_name = row
            .get("module_name")
            .and_then(|v| v.as_str())
            .ok_or(RowParseError::Missing("module_name"))?
            .to_owned();

        let status = row
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or(RowParseError::Missing("status"))?
            .to_owned();

        let pid = row
            .get("pid")
            .and_then(serde_json::Value::as_i64)
            .and_then(|i| i32::try_from(i).ok());

        let port = row
            .get("port")
            .and_then(serde_json::Value::as_i64)
            .ok_or(RowParseError::Missing("port"))
            .and_then(|i| i32::try_from(i).map_err(|_e| RowParseError::OutOfRange("port")))?;

        Ok(Self {
            name,
            module_name,
            status,
            pid,
            port,
        })
    }
}

#[async_trait]
pub trait ServiceLifecycle: Send + Sync {
    async fn get_running_services(&self) -> Result<Vec<ServiceRecord>, RepositoryError>;
    async fn mark_crashed(&self, service_name: &str) -> Result<(), RepositoryError>;
    async fn update_status(&self, service_name: &str, status: &str) -> Result<(), RepositoryError>;
}
