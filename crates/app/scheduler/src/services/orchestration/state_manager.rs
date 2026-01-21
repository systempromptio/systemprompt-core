use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::process_cleanup::ProcessCleanup;
use super::state_types::{DesiredStatus, RuntimeStatus, ServiceType};
use super::verified_state::VerifiedServiceState;
use systemprompt_database::{DatabaseProvider, DatabaseQuery, DbPool};

const FETCH_DB_SERVICES: DatabaseQuery = DatabaseQuery::new(
    "SELECT name, module_name as service_type, status, pid, port FROM services WHERE status IN \
     ('running', 'starting', 'stopped')",
);

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub service_type: ServiceType,
    pub port: u16,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct DbServiceRecord {
    pub name: String,
    pub service_type: String,
    pub status: String,
    pub pid: Option<i64>,
    pub port: i32,
}

#[derive(Debug)]
pub struct ServiceStateManager {
    db_pool: DbPool,
}

impl ServiceStateManager {
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn get_verified_states(
        &self,
        configs: &[ServiceConfig],
    ) -> Result<Vec<VerifiedServiceState>> {
        let db_services = self.fetch_db_services().await?;
        let db_by_name: HashMap<String, &DbServiceRecord> =
            db_services.iter().map(|s| (s.name.clone(), s)).collect();

        let config_names: HashSet<&String> = configs.iter().map(|c| &c.name).collect();

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
                    service_type: ServiceType::from_module_name(&db_service.service_type),
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
            Some(record) if record.status == "running" => {
                if let Some(pid) = record.pid.map(|p| p as u32) {
                    if ProcessCleanup::process_exists(pid) {
                        if self.is_port_responsive(port).await {
                            (RuntimeStatus::Running, Some(pid))
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
            Some(record) if record.status == "starting" => {
                record
                    .pid
                    .map(|p| p as u32)
                    .map_or((RuntimeStatus::Stopped, None), |pid| {
                        if ProcessCleanup::process_exists(pid) {
                            (RuntimeStatus::Starting, Some(pid))
                        } else {
                            (RuntimeStatus::Stopped, None)
                        }
                    })
            },
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
        .map(|r| r.is_ok())
        .unwrap_or(false)
    }

    async fn fetch_db_services(&self) -> Result<Vec<DbServiceRecord>> {
        let empty_params: &[&dyn systemprompt_database::ToDbValue] = &[];
        let rows = self
            .db_pool
            .as_ref()
            .fetch_all(&FETCH_DB_SERVICES, empty_params)
            .await?;

        let mut records = Vec::new();
        for row in rows {
            let name = row
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    tracing::warn!("Service record missing name field");
                    ""
                })
                .to_string();
            let service_type = row
                .get("service_type")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    tracing::warn!(service_name = %name, "Service record missing service_type field");
                    "mcp"
                })
                .to_string();
            let status = row
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    tracing::warn!(service_name = %name, "Service record missing status field");
                    "stopped"
                })
                .to_string();
            let pid = row.get("pid").and_then(serde_json::Value::as_i64);
            let port = row
                .get("port")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_else(|| {
                    tracing::warn!(service_name = %name, "Service record missing port field");
                    0
                }) as i32;

            records.push(DbServiceRecord {
                name,
                service_type,
                status,
                pid,
                port,
            });
        }

        Ok(records)
    }

    pub async fn get_services_needing_action(
        &self,
        configs: &[ServiceConfig],
    ) -> Result<Vec<VerifiedServiceState>> {
        let states = self.get_verified_states(configs).await?;
        Ok(states
            .into_iter()
            .filter(VerifiedServiceState::needs_attention)
            .collect())
    }

    pub async fn get_running_services(
        &self,
        configs: &[ServiceConfig],
    ) -> Result<Vec<VerifiedServiceState>> {
        let states = self.get_verified_states(configs).await?;
        Ok(states
            .into_iter()
            .filter(|s| s.runtime_status == RuntimeStatus::Running)
            .collect())
    }

    pub async fn get_crashed_services(
        &self,
        configs: &[ServiceConfig],
    ) -> Result<Vec<VerifiedServiceState>> {
        let states = self.get_verified_states(configs).await?;
        Ok(states
            .into_iter()
            .filter(|s| s.runtime_status == RuntimeStatus::Crashed)
            .collect())
    }
}
