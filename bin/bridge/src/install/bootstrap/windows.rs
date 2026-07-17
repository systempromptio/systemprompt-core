//! Windows post-install bootstrap steps.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(not(unix))]

use std::path::Path;
use std::process::Command;

pub(super) const fn chown_to_sudo_user_if_root(_path: &Path) {}

// `Program Files\Claude\org-plugins` is admin-write-only by default; grant the
// interactive user Modify so unelevated `bridge sync` can publish there.
pub(super) fn grant_user_modify(path: &Path) -> std::io::Result<()> {
    let user = std::env::var("USERNAME")
        .map_err(|e| std::io::Error::other(format!("USERNAME env var not set: {e}")))?;
    let path_str = path.to_string_lossy().into_owned();
    let grant_arg = format!("{user}:(OI)(CI)M");

    let output = Command::new("icacls")
        .arg(&path_str)
        .arg("/grant:r")
        .arg(&grant_arg)
        .arg("/T")
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(std::io::Error::other(format!(
            "icacls grant failed (exit {:?}): {}",
            output.status.code(),
            stderr.trim()
        )));
    }
    Ok(())
}
