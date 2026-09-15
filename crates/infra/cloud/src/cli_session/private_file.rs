//! Atomic, owner-only writes for the files that hold session tokens.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::CloudResult;

static TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

// Why: the file holds tokens, so it is created private rather than created
// with the umask and tightened afterwards — that window is world-readable.
// The directory is shared by every CLI process of one user, so the staging
// name carries the pid and a counter and the rename is what publishes it.
pub(super) fn write_private_atomic(path: &Path, content: &str) -> CloudResult<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
    let nonce = TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = dir.join(format!("{name}.{}.{nonce}.tmp", std::process::id()));

    let written = create_private(&temp_path, content).and_then(|()| {
        fs::rename(&temp_path, path)?;
        Ok(())
    });
    if written.is_err()
        && let Err(cleanup) = fs::remove_file(&temp_path)
        && cleanup.kind() != std::io::ErrorKind::NotFound
    {
        tracing::debug!(path = %temp_path.display(), error = %cleanup, "temp file not removed");
    }
    written
}

fn create_private(path: &Path, content: &str) -> CloudResult<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

pub(super) fn ensure_private_dir(dir: &Path) -> CloudResult<()> {
    fs::create_dir_all(dir)?;
    let gitignore_path = dir.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(&gitignore_path, "*\n")?;
    }
    Ok(())
}
