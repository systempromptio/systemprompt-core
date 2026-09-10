//! Verbatim pass-through of the base tree at `<from>/systemprompt/`.
//!
//! The base tree carries the platform-defined halves an Anthropic repo cannot
//! express — MCP servers, agents, gateway and governance config — and is copied
//! byte for byte. A marketplace repo may not restate what it authors in
//! Anthropic form, so any directory in [`MARKETPLACE_BUNDLE_DIRS`] found here
//! is an error rather than an overlay, as is any directory outside
//! [`BUNDLE_ALLOWED_DIRS`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_models::services::bundle::{BUNDLE_ALLOWED_DIRS, MARKETPLACE_BUNDLE_DIRS};

use crate::error::MarketplaceError;

use super::writer::Sink;

pub(super) const BASE_DIR: &str = "systemprompt";

pub(super) fn copy_base_tree(from: &Path, sink: &Sink) -> Result<Vec<String>, MarketplaceError> {
    let base = from.join(BASE_DIR);
    if !base.is_dir() {
        return Ok(Vec::new());
    }

    let read = std::fs::read_dir(&base).map_err(|e| MarketplaceError::Import {
        path: base.display().to_string(),
        message: e.to_string(),
    })?;

    let mut dirs: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in read {
        let entry = entry.map_err(|e| MarketplaceError::Import {
            path: base.display().to_string(),
            message: e.to_string(),
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !BUNDLE_ALLOWED_DIRS.contains(&name) {
            return Err(MarketplaceError::Import {
                path: path.display().to_string(),
                message: format!("'{name}' is not a services bundle directory"),
            });
        }
        if MARKETPLACE_BUNDLE_DIRS.contains(&name) {
            return Err(MarketplaceError::Import {
                path: path.display().to_string(),
                message: format!(
                    "'{name}' is authored in Anthropic form and may not also appear under \
                     {BASE_DIR}/"
                ),
            });
        }
        dirs.push((name.to_owned(), path));
    }
    dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut copied = Vec::with_capacity(dirs.len());
    for (name, path) in dirs {
        sink.copy_tree(&path, Path::new(&name))?;
        copied.push(name);
    }
    Ok(copied)
}
