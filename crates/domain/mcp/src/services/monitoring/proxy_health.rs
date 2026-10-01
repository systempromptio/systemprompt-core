//! Proxy-level health checks for MCP servers (port probe + MCP connect).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use std::time::Duration;
use systemprompt_database::ServiceRepository;
use systemprompt_identifiers::ServiceName;
use systemprompt_models::services::ServiceStatus;
use tokio::net::TcpStream;

#[derive(Debug)]
pub struct ProxyHealthCheck {
    service_repo: ServiceRepository,
}

impl ProxyHealthCheck {
    pub const fn new(service_repo: ServiceRepository) -> Self {
        Self { service_repo }
    }

    pub async fn can_route_traffic(
        &self,
        service_name: &ServiceName,
        port: u16,
    ) -> McpDomainResult<bool> {
        let Some(service) = self.service_repo.find_service_by_name(service_name).await? else {
            return Ok(false);
        };

        if service.status != ServiceStatus::Running {
            return Ok(false);
        }

        if !Self::is_port_responsive(port).await {
            self.service_repo
                .update_service_status(service_name, ServiceStatus::Stopped)
                .await?;
            return Ok(false);
        }

        if !Self::can_connect_mcp(port).await {
            self.service_repo
                .update_service_status(service_name, ServiceStatus::Error)
                .await?;
            return Ok(false);
        }

        Ok(true)
    }

    async fn is_port_responsive(port: u16) -> bool {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        tokio::time::timeout(Duration::from_millis(100), TcpStream::connect(addr))
            .await
            .is_ok_and(|connected| connected.is_ok())
    }

    async fn can_connect_mcp(port: u16) -> bool {
        use crate::services::client::validate_connection;

        match tokio::time::timeout(
            Duration::from_millis(500),
            validate_connection(&ServiceName::new("proxy_check"), "127.0.0.1", port),
        )
        .await
        {
            Ok(Ok(result)) => result.success || result.validation_type == "auth_required",
            _ => false,
        }
    }

    pub async fn list_routable_services(&self) -> McpDomainResult<Vec<RoutableService>> {
        let running_services = self.service_repo.list_all_running_services().await?;

        let mut routable = Vec::new();

        for service in running_services {
            let port = Self::parse_port_from_service(&service);
            if Self::is_port_responsive(port).await {
                routable.push(RoutableService {
                    name: service.name,
                    port,
                    pid: service.pid,
                    health: "healthy".to_owned(),
                });
            } else {
                self.service_repo
                    .update_service_status(&service.name, ServiceStatus::Stopped)
                    .await?;
            }
        }

        Ok(routable)
    }

    const fn parse_port_from_service(service: &systemprompt_database::ServiceConfig) -> u16 {
        service.port as u16
    }
}

#[derive(Debug, Clone)]
pub struct RoutableService {
    pub name: ServiceName,
    pub port: u16,
    pub pid: Option<i32>,
    pub health: String,
}
