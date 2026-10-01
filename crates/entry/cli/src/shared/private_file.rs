//! Owner-only, atomic writes for files that hold secrets.
//!
//! Every secrets document the CLI writes (`secrets.json`, signing-key seeds)
//! goes through [`write_private_atomic`]. The file is created `0600` rather
//! than written with the umask and tightened afterwards, which leaves a
//! world-readable window, and it is staged under a unique temporary name and
//! renamed into place so a crash mid-write never truncates the previous copy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};

static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

pub fn write_private_atomic(path: &Path, content: &str) -> Result<()> {
    let dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;

    let name = path
        .file_name()
        .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
    let nonce = TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = dir.join(format!(".{name}.{}.{nonce}.tmp", std::process::id()));

    let written = create_private(&temp_path, content).and_then(|()| {
        fs::rename(&temp_path, path).with_context(|| format!("Failed to write {}", path.display()))
    });
    if written.is_err()
        && let Err(cleanup) = fs::remove_file(&temp_path)
        && cleanup.kind() != std::io::ErrorKind::NotFound
    {
        tracing::debug!(path = %temp_path.display(), error = %cleanup, "temp file not removed");
    }
    written
}

fn create_private(path: &Path, content: &str) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("Failed to create {}", path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("Failed to sync {}", path.display()))?;
    Ok(())
}
