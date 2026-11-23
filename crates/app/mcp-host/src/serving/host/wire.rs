//! Wire values every hosted toolset server answers with: the cached tool
//! list, the server icons, and the client profile read from the peer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::{CacheScope, Icon, ListToolsResult, Tool};
use rmcp::service::{RequestContext, RoleServer};
use systemprompt_extension::mcp::capabilities::WEBSITE_URL;
use systemprompt_models::mcp::ClientProfile;

const TOOL_LIST_TTL_MS: u64 = 3_600_000;

#[must_use]
pub fn build_tool_list_result(tools: Vec<Tool>) -> ListToolsResult {
    ListToolsResult::with_all_items(tools)
        .with_ttl_ms(TOOL_LIST_TTL_MS)
        .with_cache_scope(CacheScope::Public)
}

#[must_use]
pub fn default_server_icons() -> Vec<Icon> {
    vec![
        Icon::new(format!("{WEBSITE_URL}/files/images/favicon-32x32.png"))
            .with_mime_type("image/png")
            .with_sizes(vec!["32x32".to_owned()]),
        Icon::new(format!("{WEBSITE_URL}/files/images/favicon-96x96.png"))
            .with_mime_type("image/png")
            .with_sizes(vec!["96x96".to_owned()]),
    ]
}

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
