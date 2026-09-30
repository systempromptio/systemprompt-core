//! Vendoring a plugin whose marketplace entry names a git source.
//!
//! Claude Code fetches `github`, `url` and `git-subdir` plugins itself at
//! install time. A services bundle cannot: it is signed file by file and boot
//! never reaches out to a plugin's upstream. So the importer fetches the
//! pinned commit here, while the bundle is being built, and hands the tree to
//! the same [`super::plugin::import_plugin`] a local plugin goes through. The
//! instance then serves upstream skills from its own tree, under the same
//! entitlement, hooks and analytics as its own.
//!
//! The fetch is [`crate::managed::GitSourceCapture`]: https only, hooks off,
//! no submodules, bounded in files and bytes. Tests substitute a capture that
//! never touches the network.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::error::MarketplaceError;
use crate::managed::{GitCaptureRequest, GitSourceCapture};

use super::anthropic::RemotePluginSource;

const UNPINNED_REFERENCE: &str = "HEAD";

pub(super) struct FetchedPlugin {
    pub dir: TempTree,
    pub commit: String,
}

pub(super) fn fetch_plugin(
    capture: &dyn GitSourceCapture,
    plugin: &str,
    source: &RemotePluginSource,
) -> Result<FetchedPlugin, MarketplaceError> {
    let reference = source
        .commit
        .as_deref()
        .or(source.reference.as_deref())
        .unwrap_or(UNPINNED_REFERENCE);
    let failed = |message: String| MarketplaceError::Import {
        path: format!("{}@{reference}", source.repository),
        message: format!("plugin '{plugin}': {message}"),
    };
    let captured = capture
        .capture(&GitCaptureRequest {
            repository: &source.repository,
            reference,
            subdirectory: source.subdirectory.as_deref(),
            root: "",
            credential: None,
        })
        .map_err(|error| failed(format!("could not fetch the upstream plugin: {error}")))?;
    if let Some(pinned) = &source.commit
        && captured.commit != *pinned
    {
        return Err(failed(format!(
            "upstream answered with commit {} for pin {pinned}",
            captured.commit
        )));
    }
    if captured.files.0.is_empty() {
        return Err(failed(format!(
            "nothing at `{}` in that commit",
            source.subdirectory.as_deref().unwrap_or("/")
        )));
    }

    let dir = TempTree::create(plugin).map_err(|e| failed(e.to_string()))?;
    for (relative, file) in &captured.files.0 {
        write_file(&dir.0.join(relative), &file.bytes, file.executable)
            .map_err(|e| failed(format!("{relative}: {e}")))?;
    }
    Ok(FetchedPlugin {
        dir,
        commit: captured.commit,
    })
}

fn write_file(path: &Path, bytes: &[u8], executable: bool) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _executable = executable;
    Ok(())
}

pub(super) struct TempTree(PathBuf);

impl TempTree {
    fn create(plugin: &str) -> std::io::Result<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let slug: String = plugin
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let path = std::env::temp_dir().join(format!(
            "systemprompt-import-{slug}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _removed = std::fs::remove_dir_all(&self.0);
    }
}
