use std::sync::Arc;
use systemprompt_core_database::{ServiceConfig, ServiceRepository};
use systemprompt_core_mcp::services::McpManager;
use systemprompt_runtime::AppContext;

use super::backend::ProxyError;

pub struct ServiceResolver;

impl ServiceResolver {
    pub async fn resolve(
        service_name: &str,
        ctx: &AppContext,
    ) -> Result<ServiceConfig, ProxyError> {
        let service_repo = ServiceRepository::new(ctx.db_pool().clone());

        let service = match service_repo.get_service_by_name(service_name).await {
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
                service: service_name.to_string(),
                status: service.status.clone(),
            });
        }

        Ok(service)
    }

    async fn attempt_restart(service_name: &str, ctx: &AppContext) -> Result<(), ProxyError> {
        let orchestrator =
            McpManager::new(Arc::new(ctx.clone())).map_err(|e| ProxyError::ServiceNotRunning {
                service: service_name.to_string(),
                status: format!("Failed to create orchestrator: {e}"),
            })?;

        match orchestrator
            .start_services(Some(service_name.to_string()))
            .await
        {
            Ok(()) => {},
            Err(e) => {
                tracing::error!(service = %service_name, error = %e, "Failed to restart service");
                return Err(ProxyError::ServiceNotRunning {
                    service: service_name.to_string(),
                    status: format!("Restart failed: {e}"),
                });
            },
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    }
}
