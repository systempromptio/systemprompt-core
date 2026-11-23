//! [`McpProcessSet`]: the MCP member's supervised process set.
//!
//! Built by the member's slot factory from sealed seeds (spawner, child
//! secret environment, system admin, and the signing keys its health checks
//! mint the health hop with) and open values (verification keys, services
//! view), it builds an [`McpOrchestrator`] per operation. On a host
//! that seeds no spawner (a child host) every operation refuses with
//! `ServiceProcessError::Unavailable`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_config::DynChildSecretEnv;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::ServiceRepository;
use systemprompt_extension::{FactoryError, Resolved};
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::Spawner;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::{ServicesView, SystemAdmin};
use systemprompt_models::mcp::McpServerType;
use systemprompt_security::keys::{SigningKeys, TokenVerificationKeys};
use systemprompt_traits::{
    RestartMode, ServiceDetail, ServiceHealth, ServiceProcessError, ServiceProcessKind,
    ServiceProcessSet, ServiceProcessStatus, ServiceRun, StatusDepth, StopMode,
};

use super::McpOrchestrator;
use super::health_probe::HealthHop;
use super::monitoring::health::HealthStatus;
use super::orchestrator::McpSpawn;
use super::registry::RegistryService;

const KIND: ServiceProcessKind = ServiceProcessKind::McpServer;

struct Inputs {
    services: ServiceRepository,
    paths: Arc<AppPaths>,
    registry: RegistryService,
    spawn: McpSpawn,
}

pub struct McpProcessSet {
    inputs: Option<Inputs>,
}

impl std::fmt::Debug for McpProcessSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpProcessSet")
            .field("available", &self.inputs.is_some())
            .finish()
    }
}

fn failed(source: impl std::error::Error + Send + Sync + 'static) -> ServiceProcessError {
    ServiceProcessError::Failed {
        kind: KIND,
        source: Box::new(source),
    }
}

const fn health(status: HealthStatus) -> ServiceHealth {
    match status {
        HealthStatus::Healthy => ServiceHealth::Healthy,
        HealthStatus::Degraded => ServiceHealth::Degraded,
        HealthStatus::Unhealthy => ServiceHealth::Unhealthy,
        HealthStatus::Unknown => ServiceHealth::Unknown,
    }
}

impl McpProcessSet {
    pub fn from_resolved(
        r: &Resolved<'_>,
        services: ServiceRepository,
    ) -> Result<Self, FactoryError> {
        let (Some(spawner), Some(secrets), Some(verification_keys), Some(signing)) = (
            r.optional::<Arc<Spawner>>(),
            r.optional::<DynChildSecretEnv>(),
            r.optional::<TokenVerificationKeys>(),
            r.optional::<SigningKeys>(),
        ) else {
            return Ok(Self { inputs: None });
        };
        let config = r.get::<Arc<Config>>()?;
        Ok(Self {
            inputs: Some(Inputs {
                services,
                paths: r.get::<Arc<AppPaths>>()?,
                registry: RegistryService::new(
                    r.get::<SystemAdmin>()?.id().clone(),
                    r.get::<ServicesView>()?,
                    Arc::clone(&config),
                ),
                spawn: McpSpawn {
                    spawner,
                    secrets,
                    verification_keys,
                    health: HealthHop::new(signing, &config.jwt_issuer),
                },
            }),
        })
    }

    fn inputs(&self) -> Result<&Inputs, ServiceProcessError> {
        self.inputs
            .as_ref()
            .ok_or(ServiceProcessError::Unavailable { kind: KIND })
    }

    fn orchestrator(&self) -> Result<McpOrchestrator, ServiceProcessError> {
        let inputs = self.inputs()?;
        McpOrchestrator::new(
            inputs.services.clone(),
            Arc::clone(&inputs.paths),
            inputs.registry.clone(),
            inputs.spawn.clone(),
        )
        .map_err(failed)
    }
}

#[async_trait]
impl ServiceProcessSet for McpProcessSet {
    fn kind(&self) -> ServiceProcessKind {
        KIND
    }

    async fn statuses(
        &self,
        depth: StatusDepth,
    ) -> Result<Vec<ServiceProcessStatus>, ServiceProcessError> {
        if depth == StatusDepth::Listed {
            let registry = &self.inputs()?.registry;
            registry.validate().map_err(failed)?;
            return Ok(registry
                .get_managed_servers()
                .map_err(failed)?
                .into_iter()
                .map(|server| ServiceProcessStatus {
                    name: ServiceName::new(server.name.as_str()),
                    pid: None,
                    port: server.port,
                    health: ServiceHealth::Unknown,
                    detail: ServiceDetail::Mcp {
                        remote: false,
                        endpoint: None,
                        tools_count: None,
                        latency_ms: None,
                        auth_required: server.oauth.required,
                    },
                })
                .collect());
        }
        Ok(self
            .orchestrator()?
            .service_statuses()
            .await
            .map_err(failed)?
            .into_iter()
            .map(|status| ServiceProcessStatus {
                name: ServiceName::new(status.name.as_str()),
                pid: status.pid,
                port: status.port,
                health: health(status.health),
                detail: ServiceDetail::Mcp {
                    remote: status.server_type == McpServerType::External,
                    endpoint: status.endpoint,
                    tools_count: status.tools_count,
                    latency_ms: status.latency_ms,
                    auth_required: status.auth_required,
                },
            })
            .collect())
    }

    async fn start(
        &self,
        name: Option<&ServiceName>,
    ) -> Result<Vec<ServiceRun>, ServiceProcessError> {
        self.orchestrator()?
            .start_services(name.cloned())
            .await
            .map_err(failed)?;
        Ok(name
            .map(|name| ServiceRun {
                name: name.clone(),
                outcome: Ok(()),
            })
            .into_iter()
            .collect())
    }

    async fn stop(
        &self,
        name: Option<&ServiceName>,
        _mode: StopMode,
    ) -> Result<(), ServiceProcessError> {
        self.orchestrator()?
            .stop_services(name.cloned())
            .await
            .map_err(failed)
    }

    async fn restart(
        &self,
        name: Option<&ServiceName>,
        mode: RestartMode,
    ) -> Result<Vec<ServiceRun>, ServiceProcessError> {
        let orchestrator = self.orchestrator()?;
        if mode == RestartMode::Rebuild {
            let restarted = orchestrator
                .build_and_restart_services(name.cloned())
                .await
                .map_err(failed)?;
            return Ok(match (restarted, name) {
                (0, _) | (_, None) => Vec::new(),
                (_, Some(name)) => vec![ServiceRun {
                    name: name.clone(),
                    outcome: Ok(()),
                }],
            });
        }
        Ok(orchestrator
            .restart_services(name.cloned())
            .await
            .map_err(failed)?
            .into_iter()
            .map(|outcome| ServiceRun {
                name: outcome.service_name,
                outcome: outcome.result.map_err(failed),
            })
            .collect())
    }
}
