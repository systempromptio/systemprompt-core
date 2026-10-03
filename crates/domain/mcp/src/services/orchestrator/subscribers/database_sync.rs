//! Event subscriber syncing MCP service state to the database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use async_trait::async_trait;

use crate::services::database::DatabaseService;
use systemprompt_manifest::services::ServiceStatus;

use super::{EventSubscriber, McpEvent};

#[derive(Debug)]
pub struct DatabaseSyncSubscriber {
    database: DatabaseService,
}

impl DatabaseSyncSubscriber {
    pub const fn new(database: DatabaseService) -> Self {
        Self { database }
    }
}

#[async_trait]
impl EventSubscriber for DatabaseSyncSubscriber {
    async fn handle(&self, event: &McpEvent) -> McpDomainResult<()> {
        match event {
            McpEvent::ServiceStarted { service_name, .. } => {
                self.database
                    .update_service_status(service_name, ServiceStatus::Running)
                    .await?;
            },
            McpEvent::ServiceFailed { service_name, .. } => {
                self.database
                    .update_service_status(service_name, ServiceStatus::Error)
                    .await?;
            },
            McpEvent::ServiceStopped { service_name, .. } => {
                self.database
                    .update_service_status(service_name, ServiceStatus::Stopped)
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
