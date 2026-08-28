//! On-disk primitives for auth setup: restricted-permission atomic writes,
//! directory creation, and removal of the per-identity state files.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SetupError;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

pub(super) fn ensure_dir(dir: &Path) -> Result<(), SetupError> {
    fs::create_dir_all(dir)
        .map_err(|e| SetupError::Io(format!("create config dir {}: {e}", dir.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dir)
            .map_err(|e| SetupError::Io(format!("stat dir: {e}")))?
            .permissions();
        perms.set_mode(0o700);
        fs::set_permissions(dir, perms).map_err(|e| SetupError::Io(format!("chmod dir: {e}")))?;
    }
    Ok(())
}

pub(super) fn write_pat_file(path: &Path, token: &str) -> Result<(), SetupError> {
    atomic_write(path, token.trim().as_bytes(), true)
}

pub(super) fn atomic_write(target: &Path, bytes: &[u8], secret: bool) -> Result<(), SetupError> {
    let parent = target
        .parent()
        .ok_or_else(|| SetupError::Path(format!("no parent dir for {}", target.display())))?;
    let tmp = parent.join(format!(
        ".{}.tmp",
        target
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| crate::brand::brand().binary_name)
    ));
    {
        let mut f = create_restricted(&tmp, secret)?;
        f.write_all(bytes)
            .map_err(|e| SetupError::Io(format!("write {}: {e}", tmp.display())))?;
        f.sync_all()
            .map_err(|e| SetupError::Io(format!("fsync {}: {e}", tmp.display())))?;
    }
    fs::rename(&tmp, target).map_err(|e| {
        SetupError::Io(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            target.display()
        ))
    })?;
    Ok(())
}

#[cfg(unix)]
fn create_restricted(path: &Path, secret: bool) -> Result<File, SetupError> {
    use std::os::unix::fs::OpenOptionsExt;
    let mode = if secret { 0o600 } else { 0o644 };
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)
        .map_err(|e| SetupError::Io(format!("open {}: {e}", path.display())))
}

#[cfg(not(unix))]
fn create_restricted(path: &Path, _secret: bool) -> Result<File, SetupError> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|e| SetupError::Io(format!("open {}: {e}", path.display())))
}

pub(super) fn remove_if_exists(path: &Path) -> Result<(), SetupError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(SetupError::Io(format!("remove {}: {e}", path.display()))),
    }
}

pub(super) fn remove_managed_mcp_fragment() -> Result<(), SetupError> {
    let Some(meta_dir) = crate::config::paths::bridge_metadata_dir() else {
        return Ok(());
    };
    remove_if_exists(&meta_dir.join(crate::config::paths::MCP_SERVERS_FRAGMENT))
}

// Why: last-sync.json and user.json describe the identity that just logged
// out. Left behind, the replay guard compares the NEXT account's first
// manifest against the previous account's version and can wedge it as a
// replay, and the user fragment keeps naming the old identity until a sync
// happens to overwrite it.
pub(super) fn remove_sync_state() -> Result<(), SetupError> {
    let Some(meta_dir) = crate::config::paths::bridge_metadata_dir() else {
        return Ok(());
    };
    remove_if_exists(&meta_dir.join(crate::config::paths::LAST_SYNC_SENTINEL))?;
    remove_if_exists(&meta_dir.join(crate::config::paths::USER_FRAGMENT))
}

pub(super) fn strip_pat_section(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_pat = false;
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_pat = trimmed == "[pat]";
            if in_pat {
                continue;
            }
        }
        if in_pat {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}
