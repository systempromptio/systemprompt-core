mod operations;
mod verification;

use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::StartupEventSender;

use crate::services::agent_orchestration::database::AgentDatabaseService;
use crate::services::agent_orchestration::event_bus::AgentEventBus;
use crate::services::agent_orchestration::events::AgentEvent;
use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult};

#[derive(Debug)]
pub struct AgentLifecycle {
    pub(crate) db_service: AgentDatabaseService,
    pub(crate) event_bus: Option<Arc<AgentEventBus>>,
}

impl AgentLifecycle {
    pub async fn new(db_pool: DbPool) -> anyhow::Result<Self> {
        use crate::repository::agent_service::AgentServiceRepository;

        let agent_service_repo = AgentServiceRepository::new(&db_pool)?;
        let db_service = AgentDatabaseService::new(agent_service_repo).await?;

        Ok(Self {
            db_service,
            event_bus: None,
        })
    }

    pub fn with_event_bus(mut self, event_bus: Arc<AgentEventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    pub(crate) fn publish_event(&self, event: AgentEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.publish(event);
        }
    }
}

pub async fn start_agent(
    pool: &DbPool,
    agent_name: &str,
    events: Option<&StartupEventSender>,
) -> OrchestrationResult<String> {
    let lifecycle = AgentLifecycle::new(pool.clone())
        .await
        .map_err(OrchestrationError::Generic)?;
    lifecycle.start_agent(agent_name, events).await
}

pub async fn enable_agent(
    pool: &DbPool,
    agent_name: &str,
    events: Option<&StartupEventSender>,
) -> OrchestrationResult<String> {
    let lifecycle = AgentLifecycle::new(pool.clone())
        .await
        .map_err(OrchestrationError::Generic)?;
    lifecycle.enable_agent(agent_name, events).await
}

pub async fn disable_agent(pool: &DbPool, agent_name: &str) -> OrchestrationResult<()> {
    let lifecycle = AgentLifecycle::new(pool.clone())
        .await
        .map_err(OrchestrationError::Generic)?;
    lifecycle.disable_agent(agent_name).await
}

pub async fn restart_agent(
    pool: &DbPool,
    agent_name: &str,
    events: Option<&StartupEventSender>,
) -> OrchestrationResult<String> {
    let lifecycle = AgentLifecycle::new(pool.clone())
        .await
        .map_err(OrchestrationError::Generic)?;
    lifecycle.restart_agent(agent_name, events).await
}
