mod agent;
mod mcp;
mod plugin;
mod skill;

use crate::http::GatewayClient;
use crate::manifest::{SignedManifest, UserInfo};
use crate::paths::{self, OrgPluginsLocation};
use std::fs;
use std::path::Path;

pub use plugin::PluginApplyOutcome as ApplyReport;

pub fn apply_manifest(
    client: &GatewayClient,
    bearer: &str,
    manifest: &SignedManifest,
    location: &OrgPluginsLocation,
) -> Result<ApplyReport, String> {
    let root = &location.path;
    let (meta_dir, staging_root) = prepare_dirs(root)?;

    let report = plugin::apply_plugins(client, bearer, manifest, root, &staging_root)?;

    let _ = fs::remove_dir_all(&staging_root);

    mcp::write_managed_mcp_fragment(&meta_dir, &manifest.managed_mcp_servers)?;
    skill::write_skills(&meta_dir, &manifest.skills)?;
    agent::write_agents(&meta_dir, &manifest.agents)?;
    write_user(&meta_dir, manifest.user.as_ref())?;

    Ok(report)
}

fn prepare_dirs(root: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    fs::create_dir_all(root).map_err(|e| format!("create {}: {e}", root.display()))?;
    let meta_dir = paths::metadata_dir(root);
    fs::create_dir_all(&meta_dir).map_err(|e| format!("create metadata dir: {e}"))?;
    let staging_root = paths::staging_dir(root);
    let _ = fs::remove_dir_all(&staging_root);
    fs::create_dir_all(&staging_root).map_err(|e| format!("create staging: {e}"))?;
    Ok((meta_dir, staging_root))
}

fn write_user(meta_dir: &Path, user: Option<&UserInfo>) -> Result<(), String> {
    let path = meta_dir.join(paths::USER_FRAGMENT);
    let bytes = match user {
        Some(u) => serde_json::to_vec_pretty(u).map_err(|e| format!("serialize user: {e}"))?,
        None => b"null".to_vec(),
    };
    fs::write(&path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}
