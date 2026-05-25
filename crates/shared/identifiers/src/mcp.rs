//! MCP-protocol identifiers (server, execution, tool-call).

crate::define_id!(AiToolCallId, schema);
crate::define_id!(McpExecutionId, generate, schema);
crate::define_id!(McpServerId, non_empty);
crate::define_id!(McpToolName, non_empty);
