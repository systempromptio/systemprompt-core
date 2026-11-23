//! [`ClientProfile`] construction from live rmcp requests and stored sessions.
//!
//! Recovered sessions rebuild the profile from the `initialize` params
//! persisted in `mcp_sessions` (the toolset host derives it from the live
//! rmcp request context). Both feed
//! [`McpToolExecutor::execute`](crate::McpToolExecutor::execute) so the
//! response builder can shape the wire per client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::InitializeRequestParams;
use serde_json::Value as JsonValue;
use systemprompt_models::mcp::ClientProfile;

// JSON: JSONB MCP `initialize` params — decoded into `InitializeRequestParams`
// here.
pub fn client_profile_from_stored(initialize_params: &JsonValue) -> ClientProfile {
    match serde_json::from_value::<InitializeRequestParams>(initialize_params.clone()) {
        Ok(params) => ClientProfile::from_initialize_params(&params),
        Err(e) => {
            tracing::warn!(error = %e, "Stored initialize params failed to parse; treating client as unknown");
            ClientProfile::unknown()
        },
    }
}
