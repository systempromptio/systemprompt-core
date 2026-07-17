//! Service resolution for the MCP proxy, with restart-on-dead-backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::{ServiceConfig, ServiceRepository};
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_runtime::AppContext;

use super::backend::ProxyError;

#[cfg(feature = "test-api")]
pub mod test_api {
    use systemprompt_database::ServiceConfig;
    use systemprompt_runtime::AppContext;

    use super::super::backend::ProxyError;

    pub async fn resolve(
        service_name: &str,
        ctx: &AppContext,
    ) -> Result<ServiceConfig, ProxyError> {
        super::ServiceResolver::resolve(service_name, ctx).await
    }
}

pub(super) struct ServiceResolver;

impl ServiceResolver {
    pub(super) async fn resolve(
        service_name: &str,
        ctx: &AppContext,
    ) -> Result<ServiceConfig, ProxyError> {
        let service_repo =
            ServiceRepository::new(ctx.db_pool()).map_err(|e| ProxyError::DatabaseError {
                service: service_name.to_owned(),
                source: e,
            })?;

        let service = match service_repo.find_service_by_name(service_name).await {
            Ok(svc) => svc,
            Err(e) => {
                tracing::error!(service = %service_name, error = %e, "Database error when looking up service");
                return Err(ProxyError::DatabaseError {
                    service: service_name.to_owned(),
                    source: e,
                });
            },
        };

        let Some(service) = service else {
            tracing::warn!(service = %service_name, "Service not found");
            return Err(ProxyError::ServiceNotFound {
                service: service_name.to_owned(),
            });
        };

        if service.status != "running" {
            if service.status == "crashed" {
                tracing::info!(service = %service_name, "Service crashed, attempting restart");

                if Self::attempt_restart(service_name, ctx).await.is_ok() {
                    tracing::info!("Service restarted successfully, retrying proxy");
                    return Box::pin(Self::resolve(service_name, ctx)).await;
                }
            }

            tracing::warn!(service = %service_name, status = %service.status, "Service not running");
            return Err(ProxyError::ServiceNotRunning {
                service: service_name.to_owned(),
                status: service.status.clone(),
            });
        }

        Ok(service)
    }

    async fn attempt_restart(service_name: &str, ctx: &AppContext) -> Result<(), ProxyError> {
        let orchestrator = McpOrchestrator::new(
            Arc::clone(ctx.db_pool()),
            Arc::clone(ctx.app_paths_arc()),
            ctx.mcp_registry().clone(),
        )
        .map_err(|e| ProxyError::ServiceNotRunning {
            service: service_name.to_owned(),
            status: format!("Failed to create orchestrator: {e}"),
        })?;

        match orchestrator
            .start_services(Some(service_name.to_owned()))
            .await
        {
            Ok(()) => {},
            Err(e) => {
                tracing::error!(service = %service_name, error = %e, "Failed to restart service");
                return Err(ProxyError::ServiceNotRunning {
                    service: service_name.to_owned(),
                    status: format!("Restart failed: {e}"),
                });
            },
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    }
}
