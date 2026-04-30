mod error;
mod plugin;
mod synthetic_plugin;

pub use error::ApplyError;
pub use synthetic_plugin::write_synthetic_plugin;

use crate::config::paths::{self, OrgPluginsLocation};
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{SignedManifest, UserInfo};
use std::fs;
use std::path::Path;

pub use plugin::PluginApplyOutcome as ApplyReport;

pub async fn apply_manifest(
    client: &GatewayClient,
    bearer: &str,
    manifest: &SignedManifest,
    location: &OrgPluginsLocation,
) -> Result<ApplyReport, ApplyError> {
    let root = &location.path;
    let (meta_dir, staging_root) = prepare_dirs(root)?;

    if let Some(reserved) = manifest
        .plugins
        .iter()
        .find(|p| p.id.as_str() == paths::SYNTHETIC_PLUGIN_NAME)
    {
        return Err(ApplyError::ReservedPluginId(reserved.id.clone()));
    }

    let report = plugin::apply_plugins(client, bearer, manifest, root, &staging_root).await?;

    let _ = fs::remove_dir_all(&staging_root);

    synthetic_plugin::write_synthetic_plugin(root, manifest)?;
    write_user(&meta_dir, manifest.user.as_ref())?;

    Ok(report)
}

fn prepare_dirs(root: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf), ApplyError> {
    fs::create_dir_all(root).map_err(|e| ApplyError::Io {
        context: format!("create {}", root.display()),
        source: e,
    })?;
    let meta_dir = paths::metadata_dir(root);
    fs::create_dir_all(&meta_dir).map_err(|e| ApplyError::Io {
        context: "create metadata dir".into(),
        source: e,
    })?;
    let staging_root = paths::staging_dir(root);
    let _ = fs::remove_dir_all(&staging_root);
    fs::create_dir_all(&staging_root).map_err(|e| ApplyError::Io {
        context: "create staging".into(),
        source: e,
    })?;
    Ok((meta_dir, staging_root))
}

fn write_user(meta_dir: &Path, user: Option<&UserInfo>) -> Result<(), ApplyError> {
    let path = meta_dir.join(paths::USER_FRAGMENT);
    let bytes = match user {
        Some(u) => serde_json::to_vec_pretty(u).map_err(|e| ApplyError::Serialize {
            what: "user".into(),
            source: e,
        })?,
        None => b"null".to_vec(),
    };
    fs::write(&path, bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}
