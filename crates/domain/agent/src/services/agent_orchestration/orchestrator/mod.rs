mod cleanup;
mod daemon;
mod status;

use anyhow::Result;
use std::sync::Arc;
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, StartupEventSender};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::services::agent_orchestration::database::AgentDatabaseService;
use crate::services::agent_orchestration::event_bus::AgentEventBus;
use crate::services::agent_orchestration::events::AgentEvent;
use crate::services::agent_orchestration::lifecycle::AgentLifecycle;
use crate::services::agent_orchestration::monitor::AgentMonitor;
use crate::services::agent_orchestration::reconciler::AgentReconciler;
use crate::services::agent_orchestration::{monitor, AgentStatus, OrchestrationResult};
use crate::state::AgentState;

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub port: u16,
}

pub struct AgentOrchestrator {
    pub(super) db_service: AgentDatabaseService,
    pub(super) lifecycle: AgentLifecycle,
    pub(super) reconciler: AgentReconciler,
    monitor: AgentMonitor,
    pub(super) monitoring_handle: Option<JoinHandle<Result<()>>>,
    pub(super) agent_state: Arc<AgentState>,
    event_bus: Arc<AgentEventBus>,
}

impl std::fmt::Debug for AgentOrchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentOrchestrator")
            .field("db_service", &self.db_service)
            .field("lifecycle", &self.lifecycle)
            .field("reconciler", &self.reconciler)
            .field("monitor", &self.monitor)
            .field("monitoring_handle", &self.monitoring_handle.is_some())
            .field("agent_state", &"<AgentState>")
            .field("event_bus", &self.event_bus)
            .finish()
    }
}

impl AgentOrchestrator {
    pub async fn new(
        agent_state: Arc<AgentState>,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<Self> {
        tracing::debug!("Initializing Agent Orchestrator");

        let db_pool = agent_state.db_pool();

        use crate::repository::agent_service::AgentServiceRepository;
        let agent_repo = AgentServiceRepository::new(db_pool.clone());

        let event_bus = Arc::new(AgentEventBus::new(100));

        let db_service = AgentDatabaseService::new(agent_repo).await?;
        let lifecycle = AgentLifecycle::new(db_pool.clone())
            .await?
            .with_event_bus(event_bus.clone());
        let reconciler = AgentReconciler::new(db_pool.clone()).await?;
        let monitor = AgentMonitor::new(db_pool.clone()).await?;

        let orchestrator = Self {
            db_service,
            lifecycle,
            reconciler,
            monitor,
            monitoring_handle: None,
            agent_state,
            event_bus,
        };

        orchestrator.startup_reconciliation(events).await?;

        tracing::debug!("Agent Orchestrator initialized");
        Ok(orchestrator)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_bus.subscribe()
    }

    pub async fn start_agent(
        &self,
        agent_id: &str,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<String> {
        self.lifecycle.start_agent(agent_id, events).await
    }

    pub async fn enable_agent(
        &self,
        agent_id: &str,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<String> {
        self.lifecycle.enable_agent(agent_id, events).await
    }

    pub async fn disable_agent(&self, agent_id: &str) -> OrchestrationResult<()> {
        self.lifecycle.disable_agent(agent_id).await
    }

    pub async fn restart_agent(
        &self,
        agent_id: &str,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<String> {
        self.lifecycle.restart_agent(agent_id, events).await
    }

    pub async fn get_status(&self, agent_id: &str) -> OrchestrationResult<AgentStatus> {
        self.db_service.get_status(agent_id).await
    }

    pub async fn list_agents(&self) -> OrchestrationResult<Vec<(String, AgentStatus)>> {
        self.db_service.list_all_agents().await
    }

    pub async fn cleanup_crashed_agents(&self) -> OrchestrationResult<u64> {
        self.db_service.cleanup_orphaned_services().await
    }

    pub async fn health_check(
        &self,
        agent_id: &str,
    ) -> OrchestrationResult<monitor::HealthCheckResult> {
        self.monitor.comprehensive_health_check(agent_id).await
    }

    pub async fn start_all(
        &self,
        events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<Vec<String>> {
        let agents = self.db_service.list_all_agents().await?;
        let mut service_ids = Vec::new();

        for (agent_id, status) in agents {
            if matches!(status, AgentStatus::Failed { .. }) {
                match self.start_agent(&agent_id, events).await {
                    Ok(service_id) => service_ids.push(service_id),
                    Err(e) => {
                        tracing::error!(agent_id = %agent_id, error = %e, "Failed to start agent")
                    },
                }
            }
        }

        Ok(service_ids)
    }

    pub async fn disable_all(&self) -> OrchestrationResult<()> {
        let agents = self.db_service.list_all_agents().await?;

        for (agent_id, _) in agents {
            if let Err(e) = self.disable_agent(&agent_id).await {
                tracing::error!(agent_id = %agent_id, error = %e, "Failed to disable agent");
            }
        }

        Ok(())
    }

    pub async fn reconcile(&self, events: Option<&StartupEventSender>) -> OrchestrationResult<()> {
        if let Some(tx) = events {
            tx.phase_started(Phase::Agents);
        }

        self.startup_reconciliation(events).await?;

        let agents = self.db_service.list_all_agents().await?;
        let running = agents
            .iter()
            .filter(|(_, s)| matches!(s, AgentStatus::Running { .. }))
            .count();
        let total = agents.len();

        if let Some(tx) = events {
            if tx
                .unbounded_send(StartupEvent::AgentReconciliationComplete { running, total })
                .is_err()
            {
                tracing::trace!("No receivers for agent reconciliation event");
            }
            tx.phase_completed(Phase::Agents);
        }

        Ok(())
    }

    pub async fn update_agent_running(
        &self,
        agent_name: &str,
        pid: u32,
        port: u16,
    ) -> OrchestrationResult<String> {
        self.db_service
            .update_agent_running(agent_name, pid, port)
            .await
    }

    pub async fn update_agent_stopped(&self, agent_name: &str) -> OrchestrationResult<()> {
        self.db_service.update_agent_stopped(agent_name).await
    }
}

impl Drop for AgentOrchestrator {
    fn drop(&mut self) {
        if let Some(handle) = self.monitoring_handle.take() {
            handle.abort();
        }
    }
}
