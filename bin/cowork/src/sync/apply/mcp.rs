use crate::manifest::ManagedMcpServer;
use crate::paths;
use std::fs;
use std::path::Path;

pub fn write_managed_mcp_fragment(
    meta_dir: &Path,
    servers: &[ManagedMcpServer],
) -> Result<(), String> {
    let out = meta_dir.join(paths::MANAGED_MCP_FRAGMENT);
    let bytes =
        serde_json::to_vec_pretty(servers).map_err(|e| format!("serialize managed-mcp: {e}"))?;
    fs::write(&out, bytes).map_err(|e| format!("write {}: {e}", out.display()))
}
