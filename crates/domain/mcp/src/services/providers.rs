use systemprompt_traits::{
    McpServerMetadata, McpServiceProvider, McpServiceProviderError, McpServiceResult,
};

use super::registry::RegistryService;
use crate::mcp_protocol_version_str;

impl McpServiceProvider for RegistryService {
    fn protocol_version(&self) -> &str {
        mcp_protocol_version_str()
    }

    fn find_server(&self, name: &str) -> McpServiceResult<Option<McpServerMetadata>> {
        Self::find_server(self, name)
            .map(|opt| {
                opt.map(|server| McpServerMetadata {
                    name: server.name.clone(),
                    endpoint: format!("/api/v1/mcp/{}/mcp", server.name),
                })
            })
            .map_err(|e| McpServiceProviderError::Internal(e.to_string()))
    }

    fn validate_registry(&self) -> McpServiceResult<()> {
        Self::validate(self).map_err(|_e| McpServiceProviderError::RegistryUnavailable)
    }
}
