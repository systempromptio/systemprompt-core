//! [`ClientProfile`] construction from live rmcp requests and stored sessions.
//!
//! Server handlers derive the profile from the rmcp request context inside
//! `call_tool`; recovered sessions rebuild it from the `initialize` params
//! persisted in `mcp_sessions`. Both feed
//! [`McpToolExecutor::execute`](crate::McpToolExecutor::execute) so the
//! response builder can shape the wire per client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::InitializeRequestParams;
use rmcp::service::{RequestContext, RoleServer};
use serde_json::Value as JsonValue;
use systemprompt_models::mcp::ClientProfile;

pub fn client_profile_from_peer(context: &RequestContext<RoleServer>) -> ClientProfile {
    ClientProfile {
        protocol_version: context.protocol_version(),
        client_name: context.client_info().map(|info| info.name),
        extensions: context
            .client_capabilities()
            .and_then(|caps| caps.extensions)
            .map(|exts| exts.keys().cloned().collect())
            .unwrap_or_default(),
    }
}

pub fn client_profile_from_stored(initialize_params: &JsonValue) -> ClientProfile {
    match serde_json::from_value::<InitializeRequestParams>(initialize_params.clone()) {
        Ok(params) => ClientProfile::from_initialize_params(&params),
        Err(e) => {
            tracing::warn!(error = %e, "Stored initialize params failed to parse; treating client as unknown");
            ClientProfile::unknown()
        },
    }
}
