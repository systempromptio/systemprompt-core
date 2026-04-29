use crate::config::paths;
use crate::gateway::manifest::ManagedMcpServer;
use std::fs;
use std::path::Path;

#[tracing::instrument(level = "debug", skip(servers), fields(count = servers.len()))]
pub fn write_managed_mcp_fragment(
    meta_dir: &Path,
    servers: &[ManagedMcpServer],
) -> Result<(), super::ApplyError> {
    let out = meta_dir.join(paths::MANAGED_MCP_FRAGMENT);
    let bytes = serde_json::to_vec_pretty(servers).map_err(|e| super::ApplyError::Serialize {
        what: "managed-mcp".into(),
        source: e,
    })?;
    fs::write(&out, bytes).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", out.display()),
        source: e,
    })
}
