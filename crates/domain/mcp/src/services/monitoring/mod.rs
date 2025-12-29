pub mod health;
pub mod proxy_health;
pub mod status;

use crate::McpServerConfig;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone)]
pub struct MonitoringManager {
    app_context: Arc<AppContext>,
}

impl MonitoringManager {
    pub const fn new(app_context: Arc<AppContext>) -> Self {
        Self { app_context }
    }

    pub async fn check_health(&self, config: &McpServerConfig) -> Result<health::HealthStatus> {
        health::check_service_health(config).await
    }

    pub async fn get_status_for_all(
        &self,
        servers: &[McpServerConfig],
    ) -> Result<HashMap<String, status::ServiceStatus>> {
        status::get_all_service_status(servers).await
    }

    pub fn display_status(
        servers: &[McpServerConfig],
        status_data: &HashMap<String, status::ServiceStatus>,
    ) {
        status::display_service_status(servers, status_data);
    }

    pub const fn app_context(&self) -> &Arc<AppContext> {
        &self.app_context
    }
}
