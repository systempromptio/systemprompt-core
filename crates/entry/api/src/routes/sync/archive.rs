//! Filesystem and tar/gzip helpers backing the cloud-sync handlers.
//!
//! These are blocking, CPU- and syscall-bound operations (directory walks,
//! gzip, tar pack/unpack) invoked from [`super::files`] inside
//! `spawn_blocking`. Extraction enforces path-traversal guards: every entry
//! must be a regular file or directory, relative, rooted under an allowed
//! directory, and resolve inside the canonical target — anything else aborts
//! the unpack.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use systemprompt_runtime::AppContext;
use tar::{Archive, Builder};

use super::types::{FileEntry, FileManifest};

const ALLOWED_DIRS: &[&str] = &[
    "agents", "skills", "content", "mcp", "ai", "config", "profiles",
];

pub(super) fn get_services_path(ctx: &AppContext) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("SYSTEMPROMPT_SERVICES_PATH") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
        return Err(format!("SYSTEMPROMPT_SERVICES_PATH does not exist: {path}"));
    }

    let services = ctx.app_paths().system().services();
    if services.exists() {
        return Ok(services.to_path_buf());
    }

    Err("Services path not configured".into())
}

pub(super) fn collect_files(
    services_path: &Path,
    directories: &[&str],
) -> Result<FileManifest, String> {
    let mut files = Vec::new();

    for dir in directories {
        if !ALLOWED_DIRS.contains(dir) {
            continue;
        }

        let dir_path = services_path.join(dir);
        if dir_path.exists() {
            collect_dir(&dir_path, services_path, &mut files)?;
        }
    }

    let mut hasher = Sha256::new();
    let mut total_size = 0u64;
    for file in &files {
        hasher.update(&file.checksum);
        total_size += file.size;
    }
    let checksum = hex::encode(hasher.finalize());

    Ok(FileManifest {
        files,
        timestamp: chrono::Utc::now(),
        checksum,
        total_size,
    })
}

fn collect_dir(dir: &Path, base: &Path, files: &mut Vec<FileEntry>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            collect_dir(&path, base, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(base)
                .map_err(|e| format!("Failed to get relative path: {e}"))?;

            let content = fs::read(&path).map_err(|e| format!("Failed to read file: {e}"))?;
            let checksum = hex::encode(Sha256::digest(&content));

            files.push(FileEntry {
                path: relative.to_string_lossy().to_string(),
                checksum,
                size: content.len() as u64,
            });
        }
    }
    Ok(())
}

pub(super) fn create_tarball(base: &Path, manifest: &FileManifest) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = Builder::new(&mut encoder);
        for file in &manifest.files {
            let full_path = base.join(&file.path);
            tar.append_path_with_name(&full_path, &file.path)
                .map_err(|e| format!("Failed to add file to tarball: {e}"))?;
        }
        tar.finish()
            .map_err(|e| format!("Failed to finish tarball: {e}"))?;
    }
    encoder
        .finish()
        .map_err(|e| format!("Failed to finish gzip: {e}"))
}

pub(super) fn extract_tarball(data: &[u8], target: &Path) -> Result<usize, String> {
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    let mut count = 0;

    let canonical_target = target
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize target: {e}"))?;

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read tarball entries: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;

        let entry_type = entry.header().entry_type();
        if !(entry_type.is_file() || entry_type.is_dir()) {
            let entry_path = entry
                .path()
                .map_err(|e| format!("Failed to get entry path: {e}"))?;
            return Err(format!(
                "Disallowed entry type {:?} in tarball: {}",
                entry_type,
                entry_path.to_string_lossy()
            ));
        }

        let entry_path = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {e}"))?;

        if entry_path.is_absolute() {
            return Err(format!(
                "Absolute path in tarball: {}",
                entry_path.to_string_lossy()
            ));
        }

        let entry_path_str = entry_path.to_string_lossy();
        if entry_path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }) {
            return Err(format!("Invalid path in tarball: {entry_path_str}"));
        }

        let first_component = entry_path
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str());

        if !first_component.is_some_and(|c| ALLOWED_DIRS.contains(&c)) {
            return Err(format!("Path not in allowed directory: {entry_path_str}"));
        }

        let dest_path = canonical_target.join(&*entry_path);

        if !dest_path.starts_with(&canonical_target) {
            return Err(format!("Path escapes target directory: {entry_path_str}"));
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
        }

        entry
            .unpack(&dest_path)
            .map_err(|e| format!("Failed to unpack file: {e}"))?;
        count += 1;
    }

    Ok(count)
}

pub(super) fn peek_manifest(data: &[u8]) -> Result<FileManifest, String> {
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    let mut files = Vec::new();
    let mut total_size = 0u64;

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read tarball: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let size = entry.size();
        total_size += size;

        files.push(FileEntry {
            path: entry
                .path()
                .map_err(|e| format!("Invalid path: {e}"))?
                .to_string_lossy()
                .to_string(),
            checksum: String::new(),
            size,
        });
    }

    Ok(FileManifest {
        files,
        timestamp: chrono::Utc::now(),
        checksum: String::new(),
        total_size,
    })
}
