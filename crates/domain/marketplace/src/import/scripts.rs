//! Verbatim copy of the scripts a plugin sidecar declares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_models::services::plugin::PluginScript;

use crate::bundle::{NODE_PACKAGE_FILE, node_lockfile};
use crate::error::MarketplaceError;

use super::writer::Sink;

pub(super) fn copy_plugin_scripts(
    plugin_id: &str,
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
        let rel = Path::new("plugins").join(plugin_id).join(&script.source);
        sink.copy_file(&src, &rel)?;
    }
    Ok(())
}

// Why: the bundle builder only ships Node files it finds beside the plugin
// config, so an imported plugin keeps its `package.json` and lockfile there.
pub(super) fn copy_node_package_files(
    plugin_id: &str,
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
        let rel = Path::new("plugins").join(plugin_id).join(name);
        sink.copy_file(&dir.join(name), &rel)?;
    }
    Ok(())
}
