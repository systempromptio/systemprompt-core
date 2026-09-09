//! Projects catalogue connector references through the signed user manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::host_sync::HostSyncCtx;
use crate::ids::PluginId;

pub(super) fn servers_for_plugin(ctx: &HostSyncCtx<'_>, id: &PluginId) -> Vec<String> {
    // Include unowned managed servers in the first plugin so unprivileged
    // installs also receive services that have no plugin reference.
    let primary = ctx.manifest.plugins.first().is_some_and(|p| p.id == *id);
    ctx.manifest
        .managed_mcp_servers
        .iter()
        .filter(|server| {
            let name = server.name.as_str();
            let referenced = ctx
                .plugin_mcp_servers
                .get(id.as_str())
                .is_some_and(|names| names.iter().any(|n| n == name));
            let unowned = !ctx
                .plugin_mcp_servers
                .values()
                .any(|names| names.iter().any(|n| n == name));
            referenced || (primary && unowned)
        })
        .map(|server| server.name.to_string())
        .collect()
}
