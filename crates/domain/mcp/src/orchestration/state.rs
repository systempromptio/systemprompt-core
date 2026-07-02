use crate::error::McpDomainResult;
use systemprompt_database::{DbPool, ServiceRepository};

use super::models::McpServiceState;

#[derive(Debug, Clone)]
pub struct ServiceStateService {
    service_repo: ServiceRepository,
}

impl ServiceStateService {
    pub fn new(db_pool: &DbPool) -> McpDomainResult<Self> {
        Ok(Self {
            service_repo: ServiceRepository::new(db_pool)?,
        })
    }

    pub async fn get_mcp_service(&self, name: &str) -> McpDomainResult<Option<McpServiceState>> {
        let service = self.service_repo.find_service_by_name(name).await?;
        Ok(service.map(|s| McpServiceState {
            name: s.name,
            host: "127.0.0.1".to_owned(),
            port: s.port as u16,
            status: s.status,
        }))
    }

    pub async fn list_mcp_services(&self) -> McpDomainResult<Vec<McpServiceState>> {
        let services = self.service_repo.list_mcp_services().await?;
        Ok(services
            .into_iter()
            .map(|s| McpServiceState {
                name: s.name,
                host: "127.0.0.1".to_owned(),
                port: s.port as u16,
                status: s.status,
            })
            .collect())
    }

    pub async fn list_running_mcp_services(&self) -> McpDomainResult<Vec<McpServiceState>> {
        let services = self.service_repo.list_mcp_services().await?;
        Ok(services
            .into_iter()
            .filter(|s| s.status == "running")
            .map(|s| McpServiceState {
                name: s.name,
                host: "127.0.0.1".to_owned(),
                port: s.port as u16,
                status: s.status,
            })
            .collect())
    }
}
