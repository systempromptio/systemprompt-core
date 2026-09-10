//! Filesystem and tar/gzip helpers backing the file-download handlers.
//!
//! The walk, hashing and packing themselves live in
//! [`systemprompt_loader::bundle::pack`], shared with the bundle publisher so
//! a downloaded tree and a published bundle are hashed the same way. These
//! are blocking, CPU- and syscall-bound operations invoked from
//! [`super::files`] inside `spawn_blocking`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_loader::bundle::pack::{collect_files, create_tarball_bytes, manifest_checksum};
use systemprompt_runtime::AppContext;

use super::types::FileManifest;

const ALLOWED_DIRS: &[&str] = &[
    "agents", "skills", "content", "mcp", "ai", "config", "profiles",
];

pub(super) fn get_services_path(ctx: &AppContext) -> Result<PathBuf, String> {
    let services = ctx.app_paths().system().services();
    if services.exists() {
        return Ok(services.to_path_buf());
    }

    Err("Services path not configured".into())
}

pub(super) fn collect_manifest(
    services_path: &Path,
    directories: &[&str],
) -> Result<FileManifest, String> {
    let requested: Vec<&str> = directories
        .iter()
        .copied()
        .filter(|d| ALLOWED_DIRS.contains(d))
        .collect();
    let files = collect_files(services_path, &requested)
        .map_err(|e| format!("Failed to walk services tree: {e}"))?;
    let (checksum, total_size) = manifest_checksum(&files);

    Ok(FileManifest {
        files,
        timestamp: chrono::Utc::now(),
        checksum,
        total_size,
    })
}

pub(super) fn create_tarball(base: &Path, manifest: &FileManifest) -> Result<Vec<u8>, String> {
    create_tarball_bytes(base, &manifest.files).map_err(|e| e.to_string())
}
