//! Capabilities our outbound MCP client declares to the servers it dials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::{
    ClientCapabilities, ElicitationCapability, ExtensionCapabilities, FormElicitationCapability,
    TASKS_EXTENSION_ID, UrlElicitationCapability,
};
use systemprompt_models::mcp::McpExtensionId;

// Why: Enterprise-Managed Authorization enables non-interactive auth
// challenges; SEP-2663 servers require clients to advertise task support before
// returning task handles.
pub(super) fn client_capabilities(with_elicitation: bool) -> ClientCapabilities {
    let mut extensions = ExtensionCapabilities::new();
    extensions.insert(
        McpExtensionId::EnterpriseManagedAuth.as_str().to_owned(),
        serde_json::Map::new(),
    );
    extensions.insert(TASKS_EXTENSION_ID.to_owned(), serde_json::Map::new());
    let builder = ClientCapabilities::builder().enable_extensions_with(extensions);
    let mut capabilities = builder.build();
    if with_elicitation {
        capabilities.elicitation = Some(
            ElicitationCapability::new()
                .with_form(FormElicitationCapability::default())
                .with_url(UrlElicitationCapability::default()),
        );
    }
    capabilities
}
