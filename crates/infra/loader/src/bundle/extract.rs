//! Guarded extraction of a services tarball.
//!
//! Hardened against path traversal: symlinks and every other non-regular
//! entry type, absolute paths, `..` components, unlisted top-level
//! directories and destinations outside the target are rejected before
//! anything touches disk. The declared uncompressed size is accumulated as
//! entries are read so a decompression bomb is refused mid-stream rather than
//! after it has filled the volume.
//!
//! [`TarLayout`] distinguishes the two archive shapes in use: a services
//! bundle carries `bundle.json` at the root with the tree under
//! [`BUNDLE_TREE_PREFIX`], while a backup archive carries the tree at the
//! root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use systemprompt_models::services::bundle::BUNDLE_MANIFEST_FILE;
use tar::Archive;

use super::error::{BundleError, BundleResult};

pub const BUNDLE_TREE_PREFIX: &str = "services";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TarLayout {
    Bundle,
    Root,
}

#[derive(Debug, Clone, Copy)]
pub struct ExtractOptions<'a> {
    pub allowed_dirs: &'a [&'a str],
    pub max_bytes: u64,
    pub layout: TarLayout,
}

pub fn extract_tarball(
    archive: &Path,
    dest: &Path,
    opts: &ExtractOptions<'_>,
) -> BundleResult<Vec<PathBuf>> {
    let file = fs::File::open(archive)?;
    extract_reader(file, dest, opts)
}

pub fn extract_bytes(
    data: &[u8],
    dest: &Path,
    opts: &ExtractOptions<'_>,
) -> BundleResult<Vec<PathBuf>> {
    extract_reader(data, dest, opts)
}

fn extract_reader<R: Read>(
    reader: R,
    dest: &Path,
    opts: &ExtractOptions<'_>,
) -> BundleResult<Vec<PathBuf>> {
    fs::create_dir_all(dest)?;
    let root = dest.canonicalize()?;
    let mut archive = Archive::new(GzDecoder::new(reader));
    let mut written = Vec::new();
    let mut budget = opts.max_bytes;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_type = entry.header().entry_type();
        let raw = entry.path()?.into_owned();

        if !(entry_type.is_file() || entry_type.is_dir()) {
            return Err(BundleError::extract(
                &raw,
                format!("disallowed entry type {entry_type:?}"),
            ));
        }
        reject_traversal(&raw)?;

        let size = entry.header().size().unwrap_or(0);
        budget = budget.checked_sub(size).ok_or(BundleError::TooLarge {
            bytes: opts.max_bytes,
        })?;

        let Some(relative) = target_path(&raw, opts)? else {
            continue;
        };
        let dest_path = root.join(&relative);
        if !dest_path.starts_with(&root) {
            return Err(BundleError::extract(&raw, "path escapes the target"));
        }

        if entry_type.is_dir() {
            fs::create_dir_all(&dest_path)?;
            continue;
        }
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        entry
            .unpack(&dest_path)
            .map_err(|e| BundleError::extract(&dest_path, e))?;
        written.push(relative);
    }

    Ok(written)
}

fn reject_traversal(path: &Path) -> BundleResult<()> {
    let traversing = path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::RootDir));
    if traversing {
        return Err(BundleError::extract(path, "invalid path in archive"));
    }
    Ok(())
}

fn target_path(raw: &Path, opts: &ExtractOptions<'_>) -> BundleResult<Option<PathBuf>> {
    let normal: Vec<&str> = raw
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();

    let Some(first) = normal.first().copied() else {
        return Ok(None);
    };

    let (stripped, rest): (PathBuf, &[&str]) = match opts.layout {
        TarLayout::Root => (normal.iter().collect(), normal.as_slice()),
        TarLayout::Bundle => {
            if normal.len() == 1 && first == BUNDLE_MANIFEST_FILE {
                return Ok(Some(PathBuf::from(BUNDLE_MANIFEST_FILE)));
            }
            if first != BUNDLE_TREE_PREFIX {
                return Err(BundleError::extract(
                    raw,
                    "bundle entries must be bundle.json or under services/",
                ));
            }
            (normal[1..].iter().collect(), &normal[1..])
        },
    };

    let Some(top) = rest.first().copied() else {
        return Ok(None);
    };
    if !opts.allowed_dirs.contains(&top) {
        return Err(BundleError::extract(
            raw,
            format!("path not in an allowed top-level directory: {top}"),
        ));
    }
    Ok(Some(stripped))
}
