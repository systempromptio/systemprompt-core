//! The rmcp [`ClientHandler`] this crate presents to MCP servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::elicitation::{self, SharedElicitationDelegate};
use rmcp::handler::client::progress::ProgressDispatcher;
use rmcp::model::{ClientInfo, ProgressNotificationParam};
use rmcp::service::NotificationContext;
use rmcp::{ClientHandler, RoleClient};

#[derive(Debug, Clone)]
pub struct McpClientHandler {
    progress_dispatcher: ProgressDispatcher,
    client_info: ClientInfo,
    elicitation: Option<SharedElicitationDelegate>,
}

impl McpClientHandler {
    pub fn new(client_info: ClientInfo) -> Self {
        Self {
            progress_dispatcher: ProgressDispatcher::new(),
            client_info,
            elicitation: None,
        }
    }

    #[must_use]
    pub fn with_elicitation(mut self, delegate: SharedElicitationDelegate) -> Self {
        self.elicitation = Some(delegate);
        self
    }

    pub const fn progress_dispatcher(&self) -> &ProgressDispatcher {
        &self.progress_dispatcher
    }
}

impl ClientHandler for McpClientHandler {
    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.progress_dispatcher.handle_notification(params).await;
    }

    async fn create_elicitation(
        &self,
        params: rmcp::model::ElicitRequestParams,
        _context: rmcp::service::RequestContext<RoleClient>,
    ) -> Result<rmcp::model::ElicitResult, rmcp::ErrorData> {
        Ok(elicitation::handle_elicitation(self.elicitation.as_ref(), params).await)
    }

    async fn on_task_status(
        &self,
        params: rmcp::model::TaskStatusNotificationParams,
        _context: NotificationContext<RoleClient>,
    ) {
        tracing::debug!(
            task_id = %params.task.task.task_id,
            status = ?params.task.task.status,
            "Task status notification received"
        );
    }

    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }
}
