//! Agent identity newtypes: opaque [`AgentId`] (UUID-backed), checked
//! [`AgentName`] (non-empty, rejects the `"unknown"`/`"unset"` sentinels), and
//! [`ExternalAgentId`] for off-platform "super-agents" (Claude Desktop,
//! Codex CLI, Claude Code) that connect via the bridge binary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(AgentId, generate, schema);
crate::define_id!(ExternalAgentId, non_empty);

use crate::error::IdValidationError;

crate::define_id!(AgentName, checked, validate_agent_name);

fn validate_agent_name(name: &str) -> Result<(), IdValidationError> {
    if name.trim().is_empty() {
        return Err(IdValidationError::empty("AgentName"));
    }
    if name.eq_ignore_ascii_case("unknown") || name.eq_ignore_ascii_case("unset") {
        return Err(IdValidationError::invalid(
            "AgentName",
            format!("'{name}' is a reserved sentinel, not an agent name"),
        ));
    }
    Ok(())
}

impl AgentName {
    pub fn system() -> Self {
        Self("system".to_owned())
    }

    pub fn bridge() -> Self {
        Self("bridge".to_owned())
    }
}
