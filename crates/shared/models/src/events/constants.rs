//! Wire-format string constants for event and protocol names.
//!
//! Grouped by protocol surface: [`a2a`] (agent-to-agent events) and
//! [`system`] (context/connection events).

pub mod a2a {
    pub const TASK_SUBMITTED: &str = "TASK_SUBMITTED";
    pub const TASK_STATUS_UPDATE: &str = "TASK_STATUS_UPDATE";
    pub const ARTIFACT_CREATED: &str = "ARTIFACT_CREATED";
    pub const ARTIFACT_UPDATED: &str = "ARTIFACT_UPDATED";
    pub const AGENT_MESSAGE: &str = "AGENT_MESSAGE";
    pub const INPUT_REQUIRED: &str = "INPUT_REQUIRED";
    pub const AUTH_REQUIRED: &str = "AUTH_REQUIRED";
    pub const JSON_RPC_RESPONSE: &str = "JSON_RPC_RESPONSE";
    pub const JSON_RPC_ERROR: &str = "JSON_RPC_ERROR";
}

pub mod system {
    pub const CONTEXT_CREATED: &str = "CONTEXT_CREATED";
    pub const CONTEXT_UPDATED: &str = "CONTEXT_UPDATED";
    pub const CONTEXT_DELETED: &str = "CONTEXT_DELETED";
    pub const CONTEXTS_SNAPSHOT: &str = "CONTEXTS_SNAPSHOT";
    pub const CONNECTED: &str = "CONNECTED";
    pub const HEARTBEAT: &str = "HEARTBEAT";
}
