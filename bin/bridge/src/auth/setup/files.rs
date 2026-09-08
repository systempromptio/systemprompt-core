//! Verified private files used by authentication setup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::SetupError;
use std::path::Path;

pub(super) fn ensure_dir(dir: &Path) -> Result<(), SetupError> {
    crate::fsutil::create_dir_all_mode_0700(dir)
        .map_err(|e| SetupError::Io(format!("prepare {}: {e}", dir.display())))
}
pub(super) fn write_pat_file(path: &Path, token: &str) -> Result<(), SetupError> {
    atomic_write(path, token.trim().as_bytes(), true)
}
pub(super) fn atomic_write(path: &Path, bytes: &[u8], secret: bool) -> Result<(), SetupError> {
    let result = if secret {
        crate::fsutil::atomic_write_0600(path, bytes)
    } else {
        crate::fsutil::atomic_write_0644(path, bytes)
    };
    result.map_err(|e| SetupError::Io(format!("write and verify {}: {e}", path.display())))
}
pub(super) fn remove_if_exists(path: &Path) -> Result<(), SetupError> {
    crate::fsutil::remove_verified(path)
        .map_err(|e| SetupError::Io(format!("remove {}: {e}", path.display())))
}

pub(super) fn remove_managed_mcp_fragment() -> Result<(), SetupError> {
    let meta_dir = crate::config::paths::bridge_metadata_dir()
        .ok_or_else(|| SetupError::Path("bridge metadata path unresolvable".into()))?;
    remove_if_exists(&meta_dir.join(crate::config::paths::MCP_SERVERS_FRAGMENT))
}

pub(super) fn remove_sync_state() -> Result<(), SetupError> {
    let meta_dir = crate::config::paths::bridge_metadata_dir()
        .ok_or_else(|| SetupError::Path("bridge metadata path unresolvable".into()))?;
    remove_if_exists(&meta_dir.join(crate::config::paths::LAST_SYNC_SENTINEL))?;
    remove_if_exists(&meta_dir.join(crate::config::paths::USER_FRAGMENT))
}
