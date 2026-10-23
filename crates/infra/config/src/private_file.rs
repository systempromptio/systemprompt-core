//! Atomic, owner-only file writes for secrets, private keys and tokens.
//!
//! The file is staged under a unique name created with mode `0600`, synced,
//! and published by `rename`, so no reader ever sees partial content and the
//! secret is never readable by other users — not even for the window a
//! write-then-chmod sequence would leave open.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::Write;
use std::path::Path;

pub fn write_private_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
    let nonce = rand::random::<u64>();
    let temp_path = dir.join(format!(".{name}.{}.{nonce}.tmp", std::process::id()));

    let written = create_private(&temp_path, content).and_then(|()| fs::rename(&temp_path, path));
    if written.is_err()
        && let Err(cleanup) = fs::remove_file(&temp_path)
        && cleanup.kind() != std::io::ErrorKind::NotFound
    {
        tracing::debug!(path = %temp_path.display(), error = %cleanup, "temp file not removed");
    }
    written
}

fn create_private(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(content)?;
    file.sync_all()
}
