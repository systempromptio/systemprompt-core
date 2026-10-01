//! Service resolution for the MCP proxy, with restart-on-dead-backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::ServiceConfig;
use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_mcp::services::spawn_target::SpawnTarget;
use systemprompt_models::services::ServiceStatus;
use systemprompt_runtime::AppContext;

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
    ) -> Result<ServiceConfig, ProxyError> {
        let service_repo = ctx.service_repository();

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

                let restart = Self::attempt_restart(service_name, ctx).await;
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

        Ok(Self::reconcile_internal_port(service_name, service, ctx).await)
    }

    async fn reconcile_internal_port(
        service_name: &ServiceName,
        mut service: ServiceConfig,
        ctx: &AppContext,
    ) -> ServiceConfig {
        let config = match ctx.mcp_registry().find_server(service_name.as_str()) {
            Ok(Some(config)) => config,
            Ok(None) => return service,
            Err(e) => {
                tracing::debug!(service = %service_name, error = %e, "Registry lookup failed while reconciling service port");
                return service;
            },
        };

        let Ok(config_port) = config.spawn_port() else {
            return service;
        };

        if let Some(new_port) = stale_port(service.port, config_port) {
            tracing::warn!(
                service = %service_name,
                db_port = service.port,
                config_port = new_port,
                "DB service port is stale; reconciling to the port this instance spawns before proxying"
            );
            if let Err(e) = ctx
                .service_repository()
                .update_service_port(service_name, new_port)
                .await
            {
                tracing::error!(service = %service_name, error = %e, "Failed to persist reconciled service port");
            }
            service.port = i32::from(new_port);
        }

        service
    }

    async fn attempt_restart(
        service_name: &ServiceName,
        ctx: &AppContext,
    ) -> Result<(), ProxyError> {
        let orchestrator = McpOrchestrator::new(
            (**ctx.service_repository()).clone(),
            Arc::clone(ctx.app_paths_arc()),
            ctx.mcp_registry().clone(),
        )
        .map_err(|source| ProxyError::RestartFailed {
            service: service_name.to_string(),
            source,
        })?;

        orchestrator
            .start_services(Some(service_name.clone()))
            .await
            .map_err(|source| ProxyError::RestartFailed {
                service: service_name.to_string(),
                source,
            })?;

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    }
}
