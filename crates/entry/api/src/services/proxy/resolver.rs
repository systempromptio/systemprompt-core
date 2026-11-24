//! Service resolution for the proxy, with restart-on-dead-backend.
//!
//! A crashed service is restarted through the process set the supervising
//! member provides for its kind; a managed MCP service's recorded port is
//! reconciled to the port this instance spawns it on, read through the MCP
//! backends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::{ServiceConfig, ServiceModule, ServiceRepository};
use systemprompt_extension::router::ProxyBackendResolver;
use systemprompt_extension::service_process::ServiceProcessSets;
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::services::ServiceStatus;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{ServiceProcessError, ServiceProcessKind};

use super::backend::ProxyError;

#[derive(Debug, Clone, Copy)]
pub struct ServiceResolver;

pub fn stale_port(db_port: i32, config_port: u16) -> Option<u16> {
    if db_port == i32::from(config_port) {
        None
    } else {
        Some(config_port)
    }
}

impl ServiceResolver {
    pub async fn resolve(
        service_name: &ServiceName,
        ctx: &AppContext,
        backends: Option<&dyn ProxyBackendResolver>,
    ) -> Result<ServiceConfig, ProxyError> {
        let service_repo = ctx.kernel().state().get::<Arc<ServiceRepository>>()?;

        let service = match service_repo.find_service_by_name(service_name).await {
            Ok(svc) => svc,
            Err(e) => {
                tracing::error!(service = %service_name, error = %e, "Database error when looking up service");
                return Err(ProxyError::DatabaseError {
                    service: service_name.to_string(),
                    source: e,
                });
            },
        };

        let Some(service) = service else {
            tracing::warn!(service = %service_name, "Service not found");
            return Err(ProxyError::ServiceNotFound {
                service: service_name.to_string(),
            });
        };

        if service.status != ServiceStatus::Running {
            if service.status == ServiceStatus::Error {
                tracing::info!(service = %service_name, "Service crashed, attempting restart");

                let restart = Self::attempt_restart(service_name, service.module_name, ctx).await;
                if let Err(error) = &restart {
                    tracing::error!(service = %service_name, error = ?error, "Failed to restart service");
                }
                if restart.is_ok() {
                    let restarted = service_repo
                        .find_service_by_name(service_name)
                        .await
                        .map_err(|e| ProxyError::DatabaseError {
                            service: service_name.to_string(),
                            source: e,
                        })?;

                    if let Some(restarted) = restarted
                        && restarted.status == ServiceStatus::Running
                    {
                        tracing::info!(service = %service_name, "Service restarted, retrying proxy");
                        return Ok(restarted);
                    }

                    tracing::warn!(
                        service = %service_name,
                        "Restart reported success but the service is not running"
                    );
                }
            }

            tracing::warn!(service = %service_name, status = %service.status, "Service not running");
            return Err(ProxyError::ServiceNotRunning {
                service: service_name.to_string(),
                status: service.status.to_string(),
            });
        }

        Ok(match backends {
            Some(backends) if service.module_name == ServiceModule::Mcp => {
                Self::reconcile_internal_port(service_name, service, ctx, backends).await
            },
            _ => service,
        })
    }

    async fn reconcile_internal_port(
        service_name: &ServiceName,
        mut service: ServiceConfig,
        ctx: &AppContext,
        backends: &dyn ProxyBackendResolver,
    ) -> ServiceConfig {
        let Some(config_port) = backends.spawn_port(service_name) else {
            return service;
        };

        if let Some(new_port) = stale_port(service.port, config_port) {
            tracing::warn!(
                service = %service_name,
                db_port = service.port,
                config_port = new_port,
                "DB service port is stale; reconciling to the port this instance spawns before proxying"
            );
            match ctx.kernel().state().get::<Arc<ServiceRepository>>() {
                Ok(repo) => {
                    if let Err(e) = repo.update_service_port(service_name, new_port).await {
                        tracing::error!(service = %service_name, error = %e, "Failed to persist reconciled service port");
                    }
                },
                Err(e) => {
                    tracing::error!(service = %service_name, error = %e, "Failed to persist reconciled service port");
                },
            }
            service.port = i32::from(new_port);
        }

        service
    }

    async fn attempt_restart(
        service_name: &ServiceName,
        module: ServiceModule,
        ctx: &AppContext,
    ) -> Result<(), ProxyError> {
        let kind = match module {
            ServiceModule::Mcp => ServiceProcessKind::McpServer,
            ServiceModule::Agent => ServiceProcessKind::Agent,
        };
        let failed = |source| ProxyError::RestartFailed {
            service: service_name.to_string(),
            source,
        };
        let sets = ctx
            .kernel()
            .state()
            .find::<Arc<ServiceProcessSets>>()
            .cloned()
            .unwrap_or_default();
        let set = sets
            .of_kind(kind)
            .ok_or(ServiceProcessError::Unavailable { kind })
            .map_err(failed)?;
        set.start(Some(service_name)).await.map_err(failed)?;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    }
}
