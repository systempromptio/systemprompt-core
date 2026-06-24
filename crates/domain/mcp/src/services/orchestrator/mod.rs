//! `McpOrchestrator` — the top-level MCP service supervisor.
//!
//! Coordinates the lifecycle, database, monitoring, network, and process layers
//! and dispatches lifecycle events through an [`EventBus`].

use crate::error::McpDomainResult;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::AppPaths;
use systemprompt_traits::StartupEventSender;

mod daemon;
pub mod event_bus;
pub mod events;
pub mod handlers;
mod lifecycle_ops;
mod process_cleanup;
mod reconciliation;
mod schema_sync;
mod server_startup;
mod service_validation;
mod target_resolution;

pub use event_bus::EventBus;
pub use events::McpEvent;
pub use handlers::{DatabaseSyncHandler, HealthCheckHandler, LifecycleHandler, MonitoringHandler};
pub use reconciliation::ReconcileParams;

use super::database::DatabaseService;
use super::lifecycle::LifecycleOrchestrator;
use super::monitoring::MonitoringService;
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
    db_pool: DbPool,
    registry: RegistryService,
}

impl McpOrchestrator {
    #[expect(
        clippy::needless_pass_by_value,
        reason = "owned RegistryService is taken so the orchestrator can store it without an \
                  extra Arc clone at the call site"
    )]
    pub fn new(
        db_pool: DbPool,
        app_paths: Arc<AppPaths>,
        registry: RegistryService,
    ) -> McpDomainResult<Self> {
        let mut event_bus = EventBus::new(100);

        registry.validate()?;
        let database = DatabaseService::new(
            Arc::clone(&db_pool),
            Arc::clone(&app_paths),
            registry.clone(),
        );
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

        event_bus.register_handler(Arc::new(LifecycleHandler::new(
            lifecycle.clone(),
            registry.clone(),
        )));

        event_bus.register_handler(Arc::new(MonitoringHandler));

        event_bus.register_handler(Arc::new(DatabaseSyncHandler::new(database.clone())));

        let health_handler = HealthCheckHandler::new().with_restart_sender(event_bus.sender());
        event_bus.register_handler(Arc::new(health_handler));

        Ok(Self {
            event_bus: Arc::new(event_bus),
            lifecycle,
            database,
            monitoring,
            db_pool,
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
        let status_data = self.monitoring.get_status_for_all(&servers).await?;
        MonitoringService::display_status(&servers, &status_data);
        Ok(())
    }

    pub async fn show_status(&self) -> McpDomainResult<()> {
        self.list_services().await
    }

    pub async fn sync_database_state(&self) -> McpDomainResult<()> {
        tracing::info!("Synchronizing service database state");
        let servers = self.registry.managed_servers()?;
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
            db_pool: &self.db_pool,
            registry: &self.registry,
            events,
        })
        .await
    }

    pub async fn validate_service(&self, service_name: &str) -> McpDomainResult<()> {
        service_validation::validate_service(service_name, &self.database, &self.registry).await
    }

    pub async fn get_running_servers(&self) -> McpDomainResult<Vec<McpServerConfig>> {
        self.database.get_running_servers().await
    }

    pub async fn get_service_info(
        &self,
        service_name: &str,
    ) -> McpDomainResult<Option<super::database::ServiceInfo>> {
        self.database.get_service_by_name(service_name).await
    }

    pub async fn run_daemon(&self) -> McpDomainResult<()> {
        daemon::run_daemon(
            &self.event_bus,
            &self.lifecycle,
            &self.database,
            &self.registry,
        )
        .await
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<McpEvent> {
        self.event_bus.subscribe()
    }
}
