//! Name of a platform-managed service — the `services.name` key shared by
//! MCP servers and agents, taken from the `services.yaml` key that declared
//! it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(ServiceName, checked, |value| {
    crate::macros::validate_non_empty("ServiceName", value)
});

impl ServiceName {
    pub fn of_agent(agent: &crate::AgentName) -> Self {
        Self(agent.as_str().to_owned())
    }

    pub fn of_mcp_server(server: &crate::McpServerId) -> Self {
        Self(server.as_str().to_owned())
    }
}
