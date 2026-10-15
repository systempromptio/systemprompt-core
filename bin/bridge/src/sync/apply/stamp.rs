//! The installed-plugin stamp: which signed digest a plugin directory holds.
//!
//! A plugin whose stamp names the manifest's `PluginEntry.sha256` is already
//! the plugin the gateway signed, so a sync skips it without reading, hashing,
//! staging or swapping a single file. The stamp is written only after the
//! plugin was promoted and its bundled `.mcp.json` stripped, and carries the
//! server names that strip recorded, since the file is gone on later runs.
//!
//! Stamps live in the bridge's metadata directory, never beside the plugins:
//! the org-plugins root is a host's own directory, read by the host app. Each
//! stamp records the root it describes, so a plugin synced into one org-plugins
//! location is never taken as installed in another.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use super::ApplyError;
use crate::fsutil::atomic_write_0644;
use crate::gateway::manifest::PluginEntry;

pub const STAMP_DIR: &str = "plugin-stamps";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginStamp {
    pub sha256: String,
    pub root: PathBuf,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
}

fn stamp_path(meta_dir: &Path, plugin_id: &str) -> PathBuf {
    meta_dir.join(STAMP_DIR).join(format!("{plugin_id}.json"))
}

/// The stamp of an installed plugin that already holds `plugin`'s digest.
#[must_use]
pub fn current(meta_dir: &Path, root: &Path, plugin: &PluginEntry) -> Option<PluginStamp> {
    if !root.join(plugin.id.as_str()).is_dir() {
        return None;
    }
    let bytes = fs::read(stamp_path(meta_dir, plugin.id.as_str())).ok()?;
    let stamp: PluginStamp = serde_json::from_slice(&bytes).ok()?;
    (stamp.sha256 == plugin.sha256.as_str() && stamp.root == root).then_some(stamp)
}

pub fn write(meta_dir: &Path, plugin_id: &str, stamp: &PluginStamp) -> Result<(), ApplyError> {
    let path = stamp_path(meta_dir, plugin_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ApplyError::Io {
            context: format!("create {}", parent.display()),
            source: e,
        })?;
    }
    let bytes = serde_json::to_vec(stamp).map_err(|e| ApplyError::Serialize {
        what: format!("sync stamp for {plugin_id}"),
        source: e,
    })?;
    atomic_write_0644(&path, &bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

/// Drops a stamp before its plugin directory changes, so an interrupted swap
/// never leaves a stamp vouching for a tree it does not describe.
pub fn clear(meta_dir: &Path, plugin_id: &str) -> Result<(), ApplyError> {
    let path = stamp_path(meta_dir, plugin_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(ApplyError::Io {
            context: format!("remove {}", path.display()),
            source: e,
        }),
    }
}
