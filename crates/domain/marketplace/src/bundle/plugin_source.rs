//! Where a plugin's own files (scripts, Node package files, `hosts/` content)
//! come from: the plugin directory on disk, or — for a managed plugin with a
//! published revision — that revision's files and nothing else, so what is
//! served is exactly what the publication names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::dev_files::DevFileFilter;
use crate::error::MarketplaceError;
use crate::managed::RevisionFiles;

pub(super) enum PluginSource<'a> {
    Disk(PathBuf),
    Managed(&'a RevisionFiles),
}

impl PluginSource<'_> {
    pub(super) fn read(&self, relative: &str) -> Result<Option<Vec<u8>>, MarketplaceError> {
        match self {
            Self::Managed(files) => Ok(files.0.get(relative).map(|file| file.bytes.clone())),
            Self::Disk(dir) => {
                let path = dir.join(relative);
                if !path.is_file() {
                    return Ok(None);
                }
                std::fs::read(&path)
                    .map(Some)
                    .map_err(|e| MarketplaceError::catalog(format!("read {}", path.display()), e))
            },
        }
    }

    pub(super) fn executable(&self, relative: &str) -> bool {
        match self {
            Self::Managed(files) => files.0.get(relative).is_some_and(|file| file.executable),
            Self::Disk(_) => false,
        }
    }

    pub(super) fn under(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, MarketplaceError> {
        let filter = DevFileFilter::defaults();
        match self {
            Self::Managed(files) => Ok(files
                .0
                .iter()
                .filter(|(path, _)| {
                    path.starts_with(&format!("{prefix}/")) && !filter.excludes(path, None, false)
                })
                .map(|(path, file)| (path.clone(), file.bytes.clone()))
                .collect()),
            Self::Disk(dir) => {
                let mut out = Vec::new();
                walk(dir, &dir.join(prefix), &filter, &mut out)?;
                out.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(out)
            },
        }
    }
}

fn walk(
    root: &Path,
    dir: &Path,
    filter: &DevFileFilter,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), MarketplaceError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir)
        .map_err(|e| MarketplaceError::catalog(format!("read {}", dir.display()), e))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| MarketplaceError::catalog(format!("read {}", dir.display()), e))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| MarketplaceError::catalog(format!("stat {}", path.display()), e))?;
        let Some(relative) = path
            .strip_prefix(root)
            .ok()
            .and_then(Path::to_str)
            .map(|r| r.replace('\\', "/"))
        else {
            continue;
        };
        if kind.is_symlink() || filter.excludes(&relative, None, kind.is_dir()) {
            continue;
        }
        if kind.is_dir() {
            walk(root, &path, filter, out)?;
        } else if kind.is_file() {
            let bytes = std::fs::read(&path)
                .map_err(|e| MarketplaceError::catalog(format!("read {}", path.display()), e))?;
            out.push((relative, bytes));
        }
    }
    Ok(())
}
