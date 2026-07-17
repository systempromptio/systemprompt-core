//! MCP-protocol identifiers (server, execution, tool-call).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(AiToolCallId, schema);
crate::define_id!(McpExecutionId, generate, schema);
crate::define_id!(McpServerId, non_empty);
crate::define_id!(McpToolName, non_empty);
