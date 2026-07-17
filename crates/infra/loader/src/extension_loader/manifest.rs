//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use systemprompt_models::ExtensionManifest;

#[derive(Debug, thiserror::Error)]
pub(super) enum ManifestLoadError {
    #[error("Failed to read manifest: {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse manifest: {path}: {source}")]
    Yaml {
        path: std::path::PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
}

pub(super) fn load_manifest(path: &Path) -> Result<ExtensionManifest, ManifestLoadError> {
    let content = fs::read_to_string(path).map_err(|source| ManifestLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    serde_yaml::from_str(&content).map_err(|source| ManifestLoadError::Yaml {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn mtime_of(path: &Path) -> Option<std::time::SystemTime> {
    match fs::metadata(path).and_then(|m| m.modified()) {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::debug!(
                path = %path.display(),
                error = %e,
                "Could not read mtime for binary; treating as unknown"
            );
            None
        },
    }
}
