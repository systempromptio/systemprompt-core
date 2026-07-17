//! Event handlers for the MCP orchestrator's [`EventBus`](super::EventBus).
//!
//! Each [`EventHandler`] reacts to a class of [`McpEvent`] — lifecycle,
//! health-check, monitoring, and database-sync — and is registered as a
//! trait object on the bus.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use async_trait::async_trait;

use super::events::McpEvent;

// `#[async_trait]` required: handlers are stored and dispatched as `Arc<dyn
// EventHandler>` in `EventBus`, so the trait must stay `dyn`-compatible.
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &McpEvent) -> McpDomainResult<()>;

    fn name(&self) -> &'static str;

    fn handles(&self, _event: &McpEvent) -> bool {
        true
    }
}

pub mod database_sync;
pub mod health_check;
pub mod lifecycle;
pub mod monitoring;

pub use database_sync::DatabaseSyncHandler;
pub use health_check::HealthCheckHandler;
pub use lifecycle::LifecycleHandler;
pub use monitoring::MonitoringHandler;
