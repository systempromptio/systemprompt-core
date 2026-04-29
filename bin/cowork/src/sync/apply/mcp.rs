use crate::config::paths;
use crate::gateway::manifest::ManagedMcpServer;
use std::fs;
use std::path::Path;

pub fn write_managed_mcp_fragment(
    meta_dir: &Path,
    servers: &[ManagedMcpServer],
) -> Result<(), super::ApplyError> {
    let out = meta_dir.join(paths::MANAGED_MCP_FRAGMENT);
    let bytes = serde_json::to_vec_pretty(servers)
        .map_err(|e| super::ApplyError::Detail(format!("serialize managed-mcp: {e}")))?;
    fs::write(&out, bytes)
        .map_err(|e| super::ApplyError::Detail(format!("write {}: {e}", out.display())))
}
