//! The link issuer a toolset hosted by the MCP host asks the API through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::McpServerId;
use systemprompt_mcp_host::HostContext;

use super::{ApiLinkIssuer, LinkIssueError, LinkIssuer};

pub fn hosted_link_issuer(
    host: &HostContext,
    server: &McpServerId,
) -> Result<Arc<dyn LinkIssuer>, LinkIssueError> {
    let issuer: Arc<dyn LinkIssuer> = Arc::new(ApiLinkIssuer::new(
        &host.config().api_internal_url,
        server.clone(),
    )?);
    Ok(issuer)
}
