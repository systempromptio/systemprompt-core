//! Service resolution for the MCP proxy, with restart-on-dead-backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::ServiceConfig;
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
        let service_repo = ctx.service_repository();

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

                // Why: re-read rather than recurse. `start_services` reports Ok
                // when it started nothing — an unregistered name filters to an
                // empty target list — so recursing on Ok alone spins forever on
                // a row that never leaves `crashed`, and a single proxied
                // request exhausts the stack and kills the process.
                if Self::attempt_restart(service_name, ctx).await.is_ok() {
                    let restarted = service_repo
                        .find_service_by_name(service_name)
                        .await
                        .map_err(|e| ProxyError::DatabaseError {
                            service: service_name.to_owned(),
                            source: e,
                        })?;

                    if let Some(restarted) = restarted
                        && restarted.status == "running"
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
                service: service_name.to_owned(),
                status: service.status.clone(),
            });
        }

        Ok(service)
    }

    async fn attempt_restart(service_name: &str, ctx: &AppContext) -> Result<(), ProxyError> {
        let orchestrator = McpOrchestrator::new(
            Arc::clone(ctx.db_pool()),
            (**ctx.service_repository()).clone(),
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
