//! Filesystem and tar/gzip helpers backing the file-download handlers.
//!
//! These are blocking, CPU- and syscall-bound operations (directory walks,
//! gzip, tar packing) invoked from [`super::files`] inside `spawn_blocking`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use systemprompt_runtime::AppContext;
use tar::Builder;

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
