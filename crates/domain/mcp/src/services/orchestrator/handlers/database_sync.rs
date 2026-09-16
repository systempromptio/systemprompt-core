//! Event handler syncing MCP service state to the database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use async_trait::async_trait;

use crate::services::database::{DatabaseService, ServiceLifecycleStatus};

use super::{EventHandler, McpEvent};

#[derive(Debug)]
pub struct DatabaseSyncHandler {
    database: DatabaseService,
}

impl DatabaseSyncHandler {
    pub const fn new(database: DatabaseService) -> Self {
        Self { database }
    }
}

#[async_trait]
impl EventHandler for DatabaseSyncHandler {
    async fn handle(&self, event: &McpEvent) -> McpDomainResult<()> {
        match event {
            McpEvent::ServiceStarted { service_name, .. } => {
                self.database
                    .update_service_status(service_name, ServiceLifecycleStatus::Running)
                    .await?;
            },
            McpEvent::ServiceFailed { service_name, .. } => {
                self.database
                    .update_service_status(service_name, ServiceLifecycleStatus::Error)
                    .await?;
            },
            McpEvent::ServiceStopped { service_name, .. } => {
                self.database
                    .update_service_status(service_name, ServiceLifecycleStatus::Stopped)
                    .await?;
            },
            _ => {},
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "database_sync"
    }
}
