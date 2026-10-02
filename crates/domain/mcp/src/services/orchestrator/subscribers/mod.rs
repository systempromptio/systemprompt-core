//! Event subscribers for the MCP orchestrator's [`EventBus`](super::EventBus).
//!
//! Each [`EventSubscriber`] reacts to a class of [`McpEvent`] — lifecycle,
//! monitoring, and database-sync — and is registered as a trait object on the
//! bus.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use async_trait::async_trait;

use super::events::McpEvent;

/// Registered on the event bus as `Arc<dyn EventSubscriber>`; `#[async_trait]`
/// keeps it object-safe.
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    async fn handle(&self, event: &McpEvent) -> McpDomainResult<()>;

    fn name(&self) -> &'static str;

    fn handles(&self, _event: &McpEvent) -> bool {
        true
    }
}

pub mod database_sync;
pub mod lifecycle;
pub mod monitoring;

pub use database_sync::DatabaseSyncSubscriber;
pub use lifecycle::LifecycleSubscriber;
pub use monitoring::MonitoringSubscriber;
