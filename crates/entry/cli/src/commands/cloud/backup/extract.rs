//! Guarded extraction of the downloaded services tarball.
//!
//! Hardened against path-traversal: symlinks, absolute paths, `..`
//! components, and entries outside the allowed top-level directories are all
//! rejected before anything touches disk.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use flate2::read::GzDecoder;
use tar::Archive;

const ALLOWED_DIRS: &[&str] = &[
    "agents", "skills", "content", "mcp", "ai", "config", "profiles",
];

pub(super) fn extract_tarball(data: &[u8], target: &Path) -> Result<usize> {
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    let mut count = 0;

    let canonical_target = target.canonicalize()?;

    for entry in archive.entries()? {
        let mut entry = entry?;

        let entry_type = entry.header().entry_type();
        if !(entry_type.is_file() || entry_type.is_dir()) {
            bail!(
                "disallowed entry type {:?} in tarball: {}",
                entry_type,
                entry.path()?.to_string_lossy()
            );
        }

        let entry_path = entry.path()?.into_owned();
        let entry_path_str = entry_path.to_string_lossy();

        if entry_path.is_absolute()
            || entry_path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::RootDir
                )
            })
        {
            bail!("invalid path in tarball: {entry_path_str}");
        }

        let first_component = entry_path
            .components()
            .find_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .unwrap_or("");
        if !ALLOWED_DIRS.contains(&first_component) {
            bail!("path not in allowed top-level directory: {entry_path_str}");
        }

        let dest_path = canonical_target.join(&entry_path);

        if !dest_path.starts_with(&canonical_target) {
            bail!("path escapes target directory: {entry_path_str}");
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        entry.unpack(&dest_path)?;
        count += 1;
    }

    Ok(count)
}
