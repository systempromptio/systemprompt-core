//! Data models for the [`super::ServiceRepository`].
//!
//! [`ServiceConfig`] is the typed registry row; `ServiceRow` is the raw
//! decode target of the `services` queries, converted through
//! `into_config` so a `module_name` or `status` no orchestrator writes surfaces
//! as `RepositoryError::Decode` instead of a string nobody matches.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_manifest::services::{ServiceModule, ServiceStatus};

use crate::error::RepositoryError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub instance_id: InstanceId,
    pub name: ServiceName,
    pub module_name: ServiceModule,
    pub status: ServiceStatus,
    pub pid: Option<i32>,
    pub port: i32,
    pub binary_mtime: Option<i64>,
    pub heartbeat_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug)]
pub(super) struct ServiceRow {
    pub instance_id: InstanceId,
    pub name: String,
    pub module_name: String,
    pub status: String,
    pub pid: Option<i32>,
    pub port: i32,
    pub binary_mtime: Option<i64>,
    pub heartbeat_at: String,
    pub created_at: String,
    pub updated_at: String,
}

impl ServiceRow {
    pub(super) fn into_config(self) -> Result<ServiceConfig, RepositoryError> {
        let module_name = self
            .module_name
            .parse::<ServiceModule>()
            .map_err(|source| RepositoryError::Decode {
                context: "services.module_name".to_owned(),
                source: Box::new(source),
            })?;
        let status =
            self.status
                .parse::<ServiceStatus>()
                .map_err(|source| RepositoryError::Decode {
                    context: "services.status".to_owned(),
                    source: Box::new(source),
                })?;
        Ok(ServiceConfig {
            instance_id: self.instance_id,
            name: ServiceName::new(self.name),
            module_name,
            status,
            pid: self.pid,
            port: self.port,
            binary_mtime: self.binary_mtime,
            heartbeat_at: self.heartbeat_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

pub(super) fn into_configs(rows: Vec<ServiceRow>) -> Result<Vec<ServiceConfig>, RepositoryError> {
    rows.into_iter().map(ServiceRow::into_config).collect()
}

#[derive(Debug)]
pub struct CreateServiceInput<'a> {
    pub name: &'a ServiceName,
    pub module_name: ServiceModule,
    pub status: ServiceStatus,
    pub port: u16,
    pub binary_mtime: Option<i64>,
}

#[derive(Debug)]
pub struct UpsertServiceProcessInput<'a> {
    pub name: &'a ServiceName,
    pub module_name: ServiceModule,
    pub pid: i32,
    pub port: u16,
    pub status: ServiceStatus,
}
