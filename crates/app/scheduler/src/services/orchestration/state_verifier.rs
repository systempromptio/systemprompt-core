//! Service-state verification — combines DB rows with live process / port
//! introspection to produce [`VerifiedServiceState`] snapshots.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::process_cleanup::ProcessCleanup;
use super::service_records::{DbServiceRecord, ServiceConfig};
use super::state_types::{DesiredStatus, RuntimeStatus, ServiceType};
use super::verified_state::VerifiedServiceState;
use crate::error::{SchedulerError, SchedulerResult};
use systemprompt_database::{DatabaseProvider, DatabaseQuery, DbPool, JsonRow};
use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_models::services::{ServiceModule, ServiceStatus};
use systemprompt_traits::RepositoryError;

const FETCH_DB_SERVICES: DatabaseQuery = DatabaseQuery::new(
    "SELECT name, module_name as service_type, status, pid, port, \
     EXTRACT(EPOCH FROM updated_at) AS updated_at_epoch FROM services WHERE instance_id \
     = $1 AND status IN ('running', 'starting', 'stopped')",
);

const STARTUP_GRACE: Duration = Duration::from_secs(45);

const fn service_type(module: ServiceModule) -> ServiceType {
    match module {
        ServiceModule::Agent => ServiceType::Agent,
        ServiceModule::Mcp => ServiceType::Mcp,
    }
}

fn required_text<'a>(row: &'a JsonRow, column: &str) -> SchedulerResult<&'a str> {
    row.get(column)
        .and_then(|v| v.as_str())
        .ok_or_else(|| invalid_row(format!("services row has no `{column}` value")))
}

fn invalid_row(message: String) -> SchedulerError {
    SchedulerError::Repository(RepositoryError::InvalidData(message))
}

fn decode_error(
    context: &str,
    source: impl std::error::Error + Send + Sync + 'static,
) -> SchedulerError {
    SchedulerError::Repository(RepositoryError::Decode {
        context: context.to_owned(),
        source: Box::new(source),
    })
}

fn decode_db_service(row: &JsonRow) -> SchedulerResult<DbServiceRecord> {
    let name = ServiceName::new(required_text(row, "name")?);
    let service_type = required_text(row, "service_type")?
        .parse::<ServiceModule>()
        .map_err(|e| decode_error("services.module_name", e))?;
    let status = required_text(row, "status")?
        .parse::<ServiceStatus>()
        .map_err(|e| decode_error("services.status", e))?;
    let pid = row.get("pid").and_then(serde_json::Value::as_i64);
    let port = row
        .get("port")
        .and_then(serde_json::Value::as_i64)
        .and_then(|p| i32::try_from(p).ok())
        .ok_or_else(|| invalid_row(format!("services row `{name}` has no valid `port` value")))?;
    let updated_at_epoch = row
        .get("updated_at_epoch")
        .and_then(serde_json::Value::as_f64);
    Ok(DbServiceRecord {
        name,
        service_type,
        status,
        pid,
        port,
        updated_at_epoch,
    })
}

fn now_epoch() -> Option<f64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs_f64())
}

pub fn is_wedged(
    port_up: bool,
    updated_at_epoch: Option<f64>,
    now_epoch: Option<f64>,
    grace: Duration,
) -> bool {
    if port_up {
        return false;
    }
    match (updated_at_epoch, now_epoch) {
        (Some(updated), Some(now)) => now - updated > grace.as_secs_f64(),
        _ => false,
    }
}

#[derive(Debug)]
pub struct ServiceStateVerifier {
    db_pool: DbPool,
    instance_id: InstanceId,
}

impl ServiceStateVerifier {
    pub const fn new(db_pool: DbPool, instance_id: InstanceId) -> Self {
        Self {
            db_pool,
            instance_id,
        }
    }

    pub async fn get_verified_states(
        &self,
        configs: &[ServiceConfig],
    ) -> SchedulerResult<Vec<VerifiedServiceState>> {
        let db_services = self.fetch_db_services().await?;
        let db_by_name: HashMap<&ServiceName, &DbServiceRecord> =
            db_services.iter().map(|s| (&s.name, s)).collect();

        let config_names: HashSet<&ServiceName> = configs.iter().map(|c| &c.name).collect();

        let mut states = Vec::new();

        for config in configs {
            let db_record = db_by_name.get(&config.name).copied();
            let state = self.verify_service(config, db_record).await;
            states.push(state);
        }

        for db_service in &db_services {
            if !config_names.contains(&db_service.name) {
                let orphan_config = ServiceConfig {
                    name: db_service.name.clone(),
                    service_type: service_type(db_service.service_type),
                    port: db_service.port as u16,
                    enabled: false,
                };
                let state = self.verify_service(&orphan_config, Some(db_service)).await;
                states.push(state);
            }
        }

        Ok(states)
    }

    async fn verify_service(
        &self,
        config: &ServiceConfig,
        db_record: Option<&DbServiceRecord>,
    ) -> VerifiedServiceState {
        let desired = if config.enabled {
            DesiredStatus::Enabled
        } else {
            DesiredStatus::Disabled
        };
        let (runtime, pid) = self.determine_runtime_status(db_record, config.port).await;

        let builder = VerifiedServiceState::builder(
            config.name.clone(),
            config.service_type,
            desired,
            runtime,
            config.port,
        );

        match pid {
            Some(p) => builder.with_pid(p).build(),
            None => builder.build(),
        }
    }

    async fn determine_runtime_status(
        &self,
        db_record: Option<&DbServiceRecord>,
        port: u16,
    ) -> (RuntimeStatus, Option<u32>) {
        match db_record {
            Some(record) if record.status == ServiceStatus::Running => {
                if let Some(pid) = record.pid.map(|p| p as u32) {
                    if ProcessCleanup::process_exists(pid) {
                        let port_up = self.is_port_responsive(port).await;
                        if port_up {
                            (RuntimeStatus::Running, Some(pid))
                        } else if is_wedged(
                            port_up,
                            record.updated_at_epoch,
                            now_epoch(),
                            STARTUP_GRACE,
                        ) {
                            tracing::warn!(
                                service = %record.name,
                                pid,
                                port,
                                "Service process is alive but its port is unresponsive past the \
                                 startup grace window; treating as crashed for restart"
                            );
                            (RuntimeStatus::Crashed, Some(pid))
                        } else {
                            (RuntimeStatus::Starting, Some(pid))
                        }
                    } else {
                        (RuntimeStatus::Crashed, None)
                    }
                } else {
                    (RuntimeStatus::Crashed, None)
                }
            },
            Some(record) if record.status == ServiceStatus::Starting => record
                .pid
                .map(|p| p as u32)
                .map_or((RuntimeStatus::Stopped, None), |pid| {
                    if ProcessCleanup::process_exists(pid) {
                        (RuntimeStatus::Starting, Some(pid))
                    } else {
                        (RuntimeStatus::Stopped, None)
                    }
                }),
            _ => ProcessCleanup::check_port(port).map_or((RuntimeStatus::Stopped, None), |pid| {
                (RuntimeStatus::Orphaned, Some(pid))
            }),
        }
    }

    async fn is_port_responsive(&self, port: u16) -> bool {
        timeout(
            Duration::from_millis(500),
            TcpStream::connect(format!("127.0.0.1:{}", port)),
        )
        .await
        .is_ok_and(|r| r.is_ok())
    }

    async fn fetch_db_services(&self) -> SchedulerResult<Vec<DbServiceRecord>> {
        let rows = self
            .db_pool
            .as_ref()
            .fetch_all(&FETCH_DB_SERVICES, &[&self.instance_id.as_str()])
            .await?;

        rows.iter().map(decode_db_service).collect()
    }


    pub async fn get_services_needing_action(
        &self,
        configs: &[ServiceConfig],
    ) -> SchedulerResult<Vec<VerifiedServiceState>> {
        let states = self.get_verified_states(configs).await?;
        Ok(states
            .into_iter()
            .filter(VerifiedServiceState::needs_attention)
            .collect())
    }

    pub async fn get_running_services(
        &self,
        configs: &[ServiceConfig],
    ) -> SchedulerResult<Vec<VerifiedServiceState>> {
        let states = self.get_verified_states(configs).await?;
        Ok(states
            .into_iter()
            .filter(|s| s.runtime_status == RuntimeStatus::Running)
            .collect())
    }

    pub async fn get_crashed_services(
        &self,
        configs: &[ServiceConfig],
    ) -> SchedulerResult<Vec<VerifiedServiceState>> {
        let states = self.get_verified_states(configs).await?;
        Ok(states
            .into_iter()
            .filter(|s| s.runtime_status == RuntimeStatus::Crashed)
            .collect())
    }
}
