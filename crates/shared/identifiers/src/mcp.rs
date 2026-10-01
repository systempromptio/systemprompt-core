//! MCP-protocol identifiers (server, execution, tool-call).
//!
//! `McpServerId` is the server's `services.yaml` key (its name) and
//! `McpToolName` a tool name as the server declares it; both are checked
//! non-empty names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(AiToolCallId, generate, schema);
crate::define_id!(McpExecutionId, generate, schema);
crate::define_id!(McpServerId, checked, |value| {
    crate::macros::validate_non_empty("McpServerId", value)
});
crate::define_id!(McpToolName, checked, |value| {
    crate::macros::validate_non_empty("McpToolName", value)
});
