//! `McpOrchestrator` — the top-level MCP service supervisor.
//!
//! Coordinates the lifecycle, database, monitoring, network, and process layers
//! and dispatches lifecycle events through an [`EventBus`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::ServiceRepository;
use systemprompt_identifiers::ServiceName;
use systemprompt_traits::StartupEventSender;

pub mod event_bus;
pub mod events;
pub mod handlers;
mod lifecycle_ops;
pub mod process_cleanup;
mod reconciliation;
mod server_startup;
mod service_validation;
mod target_resolution;

pub use event_bus::EventBus;
pub use events::McpEvent;
pub use handlers::{DatabaseSyncHandler, LifecycleHandler, MonitoringHandler};
pub use lifecycle_ops::McpRestartOutcome;
pub use reconciliation::ReconcileParams;

use super::database::DatabaseService;
use super::lifecycle::LifecycleOrchestrator;
use super::monitoring::MonitoringService;
use super::monitoring::status::McpServiceStatus;
use super::network::NetworkService;
use super::process::ProcessService;
use super::registry::RegistryService;
use crate::McpServerConfig;

#[derive(Debug)]
pub struct McpOrchestrator {
    event_bus: Arc<EventBus>,
    lifecycle: LifecycleOrchestrator,
    database: DatabaseService,
    monitoring: MonitoringService,
    registry: RegistryService,
}

impl McpOrchestrator {
    #[expect(
        clippy::needless_pass_by_value,
        reason = "owned RegistryService is taken so the orchestrator can store it without an \
                  extra Arc clone at the call site"
    )]
    pub fn new(
        service_repo: ServiceRepository,
        app_paths: Arc<AppPaths>,
        registry: RegistryService,
    ) -> McpDomainResult<Self> {
        let mut event_bus = EventBus::new(100);

        registry.validate()?;
        let database = DatabaseService::new(service_repo, Arc::clone(&app_paths), registry.clone());
        let network = NetworkService::new();
        let process = ProcessService::new();
        let monitoring = MonitoringService::new();
        let lifecycle = LifecycleOrchestrator::new(
            process,
            network,
            database.clone(),
            monitoring,
            Arc::clone(&app_paths),
        );

        event_bus.register_handler(Arc::new(LifecycleHandler));

        event_bus.register_handler(Arc::new(MonitoringHandler));

        event_bus.register_handler(Arc::new(DatabaseSyncHandler::new(database.clone())));

        Ok(Self {
            event_bus: Arc::new(event_bus),
            lifecycle,
            database,
            monitoring,
            registry,
        })
    }

    pub const fn registry(&self) -> &RegistryService {
        &self.registry
    }

    pub(super) fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub(super) const fn lifecycle(&self) -> &LifecycleOrchestrator {
        &self.lifecycle
    }

    pub(super) const fn database(&self) -> &DatabaseService {
        &self.database
    }

    pub async fn list_services(&self) -> McpDomainResult<()> {
        let servers = self.registry.get_enabled_servers()?;
        let statuses = self.monitoring.get_status_for_all(&servers).await?;
        MonitoringService::display_status(&statuses);
        Ok(())
    }

    pub async fn service_statuses(&self) -> McpDomainResult<Vec<McpServiceStatus>> {
        use super::monitoring::health::perform_health_check;

        let servers = self.registry.get_enabled_servers()?;
        let mut statuses = Vec::with_capacity(servers.len());

        for server in &servers {
            let health = perform_health_check(server).await?;
            let pid = if server.is_external() {
                None
            } else {
                self.database
                    .get_service_by_name(&server.service_name())
                    .await?
                    .and_then(|info| super::database::stored_pid(info.pid))
            };
            statuses.push(McpServiceStatus::observed(server, &health, pid));
        }

        Ok(statuses)
    }

    pub async fn show_status(&self) -> McpDomainResult<()> {
        self.list_services().await
    }

    pub async fn sync_database_state(&self) -> McpDomainResult<()> {
        tracing::info!("Synchronizing service database state");
        let servers = self.registry.get_managed_servers()?;
        self.database.sync_state(&servers).await
    }

    pub async fn reconcile(&self) -> McpDomainResult<usize> {
        self.reconcile_with_events(None).await
    }

    pub async fn reconcile_with_events(
        &self,
        events: Option<&StartupEventSender>,
    ) -> McpDomainResult<usize> {
        reconciliation::reconcile(ReconcileParams {
            database: &self.database,
            lifecycle: &self.lifecycle,
            event_bus: &self.event_bus,
            registry: &self.registry,
            events,
        })
        .await
    }

    pub async fn validate_service(&self, service_name: &ServiceName) -> McpDomainResult<()> {
        service_validation::validate_service(service_name, &self.database, &self.registry).await
    }

    pub async fn get_running_servers(&self) -> McpDomainResult<Vec<McpServerConfig>> {
        self.database.get_running_servers().await
    }

    pub async fn get_service_info(
        &self,
        service_name: &ServiceName,
    ) -> McpDomainResult<Option<super::database::ServiceInfo>> {
        self.database.get_service_by_name(service_name).await
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<McpEvent> {
        self.event_bus.subscribe()
    }
}
