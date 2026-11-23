//! [`AgentProcessSet`]: the agent member's supervised process set.
//!
//! Built by the member's slot factory from sealed seeds (spawner, child
//! secret environment) and open values (verification keys, services view), it
//! builds an [`AgentOrchestrator`] per operation, as the kernel commands did.
//! On a host that seeds no spawner (a child host) every operation refuses
//! with `ServiceProcessError::Unavailable`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_config::DynChildSecretEnv;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_extension::{FactoryError, Resolved};
use systemprompt_identifiers::{AgentName, InstanceId, ServiceName};
use systemprompt_loader::subprocess::Spawner;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_security::keys::{TokenVerificationKeys, VerificationKeyring};
use systemprompt_traits::{
    DynManagedSkillResolver, DynSessionUsageCounters, DynToolExecutionLookup, RestartMode,
    ServiceProcessError, ServiceProcessKind, ServiceProcessSet, ServiceProcessStatus, ServiceRun,
    StatusDepth, StopMode,
};

use super::AgentOrchestrator;
use super::process::SpawnEnvironment;
use super::status_view::AgentStatusView;
use crate::repository::{A2ARepositories, A2aDependencies};
use crate::services::a2a_server::streaming::webhook_client::HttpWebhookBroadcaster;
use crate::services::registry::AgentRegistry;
use crate::{AgentAuth, AgentState};

const KIND: ServiceProcessKind = ServiceProcessKind::Agent;

struct Inputs {
    view: Arc<AgentStatusView>,
    state: Arc<AgentState>,
    paths: Arc<AppPaths>,
    spawner: Arc<Spawner>,
    environment: Arc<SpawnEnvironment>,
    services: ServicesView,
}

pub struct AgentProcessSet {
    inputs: Option<Inputs>,
}

impl std::fmt::Debug for AgentProcessSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentProcessSet")
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

fn factory_failed(source: impl std::error::Error + Send + Sync + 'static) -> FactoryError {
    FactoryError::Failed(Box::new(source))
}

impl AgentProcessSet {
    pub fn from_resolved(r: &Resolved<'_>) -> Result<Self, FactoryError> {
        let (Some(spawner), Some(secrets), Some(authenticator), Some(keyring), Some(keys)) = (
            r.optional::<Arc<Spawner>>(),
            r.optional::<DynChildSecretEnv>(),
            r.optional::<DynRequestAuthenticator>(),
            r.optional::<VerificationKeyring>(),
            r.optional::<TokenVerificationKeys>(),
        ) else {
            return Ok(Self { inputs: None });
        };
        let (Some(session_usage), Some(managed_skills), Some(tool_executions)) = (
            r.optional::<DynSessionUsageCounters>(),
            r.optional::<DynManagedSkillResolver>(),
            r.optional::<DynToolExecutionLookup>(),
        ) else {
            return Ok(Self { inputs: None });
        };
        let db = r.get::<DbPool>()?;
        let config = r.get::<Arc<Config>>()?;
        let services = r.get::<ServicesView>()?;
        let repositories = A2ARepositories::new(
            &db,
            A2aDependencies {
                session_usage,
                instance_id: r.get::<InstanceId>()?,
                managed_skills,
                tool_executions,
            },
        );
        let webhooks = HttpWebhookBroadcaster::from_config(&config).map_err(factory_failed)?;
        let state = AgentState::new(
            db,
            Arc::clone(&config),
            AgentAuth {
                authenticator,
                verification_keys: keyring,
            },
            Arc::new(repositories),
            Arc::new(webhooks),
            services.clone(),
        );
        Ok(Self {
            inputs: Some(Inputs {
                view: r.get::<Arc<AgentStatusView>>()?,
                state: Arc::new(state),
                paths: r.get::<Arc<AppPaths>>()?,
                spawner,
                environment: Arc::new(SpawnEnvironment {
                    config,
                    secrets,
                    verification_keys: keys,
                    services: services.clone(),
                }),
                services,
            }),
        })
    }

    fn inputs(&self) -> Result<&Inputs, ServiceProcessError> {
        self.inputs
            .as_ref()
            .ok_or(ServiceProcessError::Unavailable { kind: KIND })
    }

    async fn orchestrator(&self) -> Result<AgentOrchestrator, ServiceProcessError> {
        let inputs = self.inputs()?;
        AgentOrchestrator::new(
            Arc::clone(&inputs.state),
            Arc::clone(&inputs.paths),
            Arc::clone(&inputs.spawner),
            Arc::clone(&inputs.environment),
            None,
        )
        .await
        .map_err(failed)
    }

    fn resolve(&self, name: &ServiceName) -> Result<AgentName, ServiceProcessError> {
        let registry = AgentRegistry::new(&self.inputs()?.services);
        registry
            .get_agent(name.as_str())
            .ok()
            .map(|agent| AgentName::new(agent.name))
            .ok_or_else(|| ServiceProcessError::Unknown {
                kind: KIND,
                name: name.clone(),
            })
    }

    fn named<'n>(
        name: Option<&'n ServiceName>,
        operation: &'static str,
    ) -> Result<&'n ServiceName, ServiceProcessError> {
        name.ok_or(ServiceProcessError::Unsupported {
            kind: KIND,
            operation,
        })
    }
}

#[async_trait]
impl ServiceProcessSet for AgentProcessSet {
    fn kind(&self) -> ServiceProcessKind {
        KIND
    }

    async fn statuses(
        &self,
        _depth: StatusDepth,
    ) -> Result<Vec<ServiceProcessStatus>, ServiceProcessError> {
        self.inputs()?.view.statuses().await.map_err(failed)
    }

    async fn start(
        &self,
        name: Option<&ServiceName>,
    ) -> Result<Vec<ServiceRun>, ServiceProcessError> {
        let agent = self.resolve(Self::named(name, "start every agent")?)?;
        let outcome = self
            .orchestrator()
            .await?
            .start_agent(&agent, None)
            .await
            .map(drop)
            .map_err(failed);
        Ok(vec![ServiceRun {
            name: ServiceName::new(agent.as_str()),
            outcome,
        }])
    }

    async fn stop(
        &self,
        name: Option<&ServiceName>,
        mode: StopMode,
    ) -> Result<(), ServiceProcessError> {
        let agent = self.resolve(Self::named(name, "stop every agent")?)?;
        let orchestrator = self.orchestrator().await?;
        match mode {
            StopMode::Disable => orchestrator.disable_agent(&agent).await,
            StopMode::Remove => orchestrator.delete_agent(&agent).await,
        }
        .map_err(failed)
    }

    async fn restart(
        &self,
        name: Option<&ServiceName>,
        mode: RestartMode,
    ) -> Result<Vec<ServiceRun>, ServiceProcessError> {
        if mode == RestartMode::Rebuild {
            return Err(ServiceProcessError::Unsupported {
                kind: KIND,
                operation: "rebuild",
            });
        }
        let agent = self.resolve(Self::named(name, "restart every agent")?)?;
        let outcome = self
            .orchestrator()
            .await?
            .restart_agent(&agent, None)
            .await
            .map(drop)
            .map_err(failed);
        Ok(vec![ServiceRun {
            name: ServiceName::new(agent.as_str()),
            outcome,
        }])
    }
}
