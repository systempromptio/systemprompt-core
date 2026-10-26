//! Verbatim copy of the plugin files the bundle ships as they are: declared
//! scripts, Node package files and the `hosts/` tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::PluginId;
use systemprompt_manifest::services::plugin::PluginScript;

use crate::bundle::{NODE_PACKAGE_FILE, node_lockfile};
use crate::error::MarketplaceError;

use super::writer::Sink;

const PLUGIN_HOSTS_DIR: &str = "hosts";

pub(super) fn copy_plugin_scripts(
    plugin_id: &PluginId,
    dir: &Path,
    scripts: &[PluginScript],
    sink: &Sink,
) -> Result<(), MarketplaceError> {
    for script in scripts {
        let src = dir.join(&script.source);
        if !src.is_file() {
            return Err(MarketplaceError::Import {
                path: src.display().to_string(),
                message: format!(
                    "plugin '{plugin_id}' declares script '{}' but the file is missing",
                    script.name
                ),
            });
        }
        let rel = Path::new("plugins")
            .join(plugin_id.as_str())
            .join(&script.source);
        sink.copy_file(&src, &rel)?;
    }
    Ok(())
}

// Why: the bundle builder only ships Node files it finds beside the plugin
// config, so an imported plugin keeps its `package.json` and lockfile there.
pub(super) fn copy_node_package_files(
    plugin_id: &PluginId,
    dir: &Path,
    sink: &Sink,
) -> Result<(), MarketplaceError> {
    if !dir.join(NODE_PACKAGE_FILE).is_file() {
        return Ok(());
    }
    let Some(lockfile) = node_lockfile(dir) else {
        return Ok(());
    };
    for name in [NODE_PACKAGE_FILE, lockfile] {
        let rel = Path::new("plugins").join(plugin_id.as_str()).join(name);
        sink.copy_file(&dir.join(name), &rel)?;
    }
    Ok(())
}

// Why: the bundle builder ships a plugin's `hosts/<host>/…` tree verbatim, so
// an imported plugin keeps it beside its config.
pub(super) fn copy_host_files(
    plugin_id: &PluginId,
    dir: &Path,
    sink: &Sink,
) -> Result<(), MarketplaceError> {
    let hosts = dir.join(PLUGIN_HOSTS_DIR);
    if !hosts.is_dir() {
        return Ok(());
    }
    let rel = Path::new("plugins")
        .join(plugin_id.as_str())
        .join(PLUGIN_HOSTS_DIR);
    sink.copy_tree(&hosts, &rel)
}
