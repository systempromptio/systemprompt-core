use systemprompt_traits::{
    McpServerMetadata, McpServiceProvider, McpServiceProviderError, McpServiceResult,
};

use super::registry::McpServerRegistry;
use crate::mcp_protocol_version;

impl McpServiceProvider for McpServerRegistry {
    fn protocol_version(&self) -> &str {
        static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        VERSION.get_or_init(mcp_protocol_version)
    }

    fn find_server(&self, name: &str) -> McpServiceResult<Option<McpServerMetadata>> {
        Self::find_server(name)
            .map(|opt| {
                opt.map(|server| McpServerMetadata {
                    name: server.name.clone(),
                    endpoint: format!("/api/v1/mcp/{}/mcp", server.name),
                })
            })
            .map_err(|e| McpServiceProviderError::Internal(e.to_string()))
    }

    fn validate_registry(&self) -> McpServiceResult<()> {
        Self::validate()
            .map_err(|_| McpServiceProviderError::RegistryUnavailable)
    }
}
