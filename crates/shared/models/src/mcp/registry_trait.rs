use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::ai::tools::McpTool;
use crate::execution::context::RequestContext;
use crate::ServerManifest;

#[derive(Debug, Clone)]
pub struct McpServerState {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub status: String,
}

#[async_trait]
pub trait McpRegistry: Send + Sync {
    async fn list_servers(&self) -> Result<Vec<String>>;

    async fn get_server_manifest(&self, name: &str) -> Result<Option<ServerManifest>>;

    async fn find_server(&self, name: &str) -> Result<Option<McpServerState>>;

    async fn server_exists(&self, name: &str) -> Result<bool>;
}

#[async_trait]
pub trait McpToolProvider: Send + Sync {
    async fn list_tools(&self, server_name: &str, context: &RequestContext)
        -> Result<Vec<McpTool>>;

    async fn load_tools_for_servers(
        &self,
        server_names: &[String],
        context: &RequestContext,
    ) -> Result<HashMap<String, Vec<McpTool>>>;
}

#[async_trait]
pub trait McpDeploymentProvider: Send + Sync {
    async fn load_config(&self) -> Result<crate::ServicesConfig>;

    fn protocol_version(&self) -> &str;
}

#[async_trait]
pub trait McpProvider: McpRegistry + McpToolProvider + McpDeploymentProvider {}

pub type DynMcpRegistry = Arc<dyn McpRegistry>;

pub type DynMcpToolProvider = Arc<dyn McpToolProvider>;

pub type DynMcpDeploymentProvider = Arc<dyn McpDeploymentProvider>;
