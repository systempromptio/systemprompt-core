//! Verbatim copy of the scripts a plugin sidecar declares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_models::services::plugin::PluginScript;

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
