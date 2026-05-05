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

use super::database::DatabaseManager;
use super::lifecycle::LifecycleManager;
use super::monitoring::MonitoringManager;
use super::network::NetworkManager;
use super::process::ProcessManager;
use super::registry::RegistryManager;
use crate::McpServerConfig;

#[derive(Debug)]
pub struct McpOrchestrator {
    event_bus: Arc<EventBus>,
    lifecycle: LifecycleManager,
    database: DatabaseManager,
    monitoring: MonitoringManager,
    db_pool: DbPool,
}

impl McpOrchestrator {
    // reason: `Arc<AppPaths>` is moved into the orchestrator; pass-by-value is the
    // intended ownership transfer
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(db_pool: DbPool, app_paths: Arc<AppPaths>) -> McpDomainResult<Self> {
        let mut event_bus = EventBus::new(100);

        RegistryManager::validate()?;
        let database = DatabaseManager::new(Arc::clone(&db_pool), Arc::clone(&app_paths));
        let network = NetworkManager::new();
        let process = ProcessManager::new();
        let monitoring = MonitoringManager::new();
        let lifecycle = LifecycleManager::new(
            process,
            network,
            database.clone(),
            monitoring,
            Arc::clone(&app_paths),
        );

        event_bus.register_handler(Arc::new(LifecycleHandler::new(lifecycle.clone())));

        event_bus.register_handler(Arc::new(MonitoringHandler::new(Arc::clone(&db_pool))));

        event_bus.register_handler(Arc::new(DatabaseSyncHandler::new(database.clone())));

        let health_handler = HealthCheckHandler::new().with_restart_sender(event_bus.sender());
        event_bus.register_handler(Arc::new(health_handler));

        Ok(Self {
            event_bus: Arc::new(event_bus),
            lifecycle,
            database,
            monitoring,
            db_pool,
        })
    }

    pub(super) fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    pub(super) const fn lifecycle(&self) -> &LifecycleManager {
        &self.lifecycle
    }

    pub(super) const fn database(&self) -> &DatabaseManager {
        &self.database
    }

    pub async fn list_services(&self) -> McpDomainResult<()> {
        let servers = RegistryManager::get_enabled_servers()?;
        let status_data = self.monitoring.get_status_for_all(&servers).await?;
        MonitoringManager::display_status(&servers, &status_data);
        Ok(())
    }

    pub async fn show_status(&self) -> McpDomainResult<()> {
        self.list_services().await
    }

    pub async fn sync_database_state(&self) -> McpDomainResult<()> {
        tracing::info!("Synchronizing service database state");
        let servers = RegistryManager::get_enabled_servers()?;
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
            events,
        })
        .await
    }

    pub async fn validate_service(&self, service_name: &str) -> McpDomainResult<()> {
        service_validation::validate_service(service_name, &self.database).await
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
        daemon::run_daemon(&self.event_bus, &self.lifecycle, &self.database).await
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<McpEvent> {
        self.event_bus.subscribe()
    }
}
