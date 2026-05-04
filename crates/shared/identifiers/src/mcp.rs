//! MCP-protocol identifiers (server, execution, tool-call).

crate::define_id!(AiToolCallId, schema);
crate::define_id!(McpExecutionId, generate, schema);
crate::define_id!(McpServerId, non_empty);

impl McpServerId {
    /// Reads the server identifier from the `MCP_SERVICE_ID` environment
    /// variable.
    pub fn from_env() -> Result<Self, crate::error::IdValidationError> {
        let id = std::env::var("MCP_SERVICE_ID").map_err(|_| {
            crate::error::IdValidationError::invalid(
                "McpServerId",
                "MCP_SERVICE_ID environment variable not set",
            )
        })?;
        Self::try_new(id)
    }
}
