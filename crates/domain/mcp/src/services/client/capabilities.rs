//! Capabilities our outbound MCP client declares to the servers it dials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::{ClientCapabilities, ExtensionCapabilities};
use systemprompt_models::mcp::McpExtensionId;

// Why: Declaring Enterprise-Managed Authorization tells a server it may answer
// an unauthenticated call with an EMA challenge rather than driving us into
// an interactive authorization redirect we have no user present to
// complete.
pub(super) fn client_capabilities() -> ClientCapabilities {
    let mut extensions = ExtensionCapabilities::new();
    extensions.insert(
        McpExtensionId::EnterpriseManagedAuth.as_str().to_owned(),
        serde_json::Map::new(),
    );
    ClientCapabilities::builder()
        .enable_extensions_with(extensions)
        .build()
}
