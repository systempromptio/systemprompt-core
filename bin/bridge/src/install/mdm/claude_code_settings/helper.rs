//! The `apiKeyHelper` the bridge hands Claude Code: a `sh` script that
//! prints the loopback key on Unix, the bridge executable itself on Windows,
//! and the shell command line each is invoked with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use super::io_error;
#[cfg(unix)]
use super::write_verified;
use crate::fsutil::FileReceipt;
use crate::install::mdm::MdmError;

#[must_use]
pub fn windows_helper_command(executable: &Path) -> String {
    let path = executable.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference = 'Stop'; & '{path}' credential-helper --host claude-code; exit $LASTEXITCODE"
    );
    // Why: -EncodedCommand accepts UTF-16LE and avoids expansion by the shell
    // Claude Code uses to launch helpers, including Git Bash and cmd.exe; the
    // same shells differ in how they expand %SystemRoot%, so the binary is
    // named bare and resolved by the launching shell.
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    format!(
        "powershell.exe -NoProfile -NonInteractive -EncodedCommand {}",
        STANDARD.encode(bytes)
    )
}

#[cfg(unix)]
pub(super) fn key_helper_path() -> Option<PathBuf> {
    Some(
        crate::basedirs::config_dir()?
            .join(crate::brand::brand().config_dir)
            .join("claude-key-helper.sh"),
    )
}

#[cfg(unix)]
fn key_helper_body(key_path: &Path) -> String {
    let bin = crate::brand::brand().binary_name;
    format!(
        "#!/bin/sh\n\
         # Written by `{bin} install --apply`. Rewritten on every apply — do not edit.\n\
         exec cat \"{key}\"\n",
        key = key_path.display(),
    )
}

// Why: Claude Code hands `apiKeyHelper` to `/bin/sh` verbatim, so a path with
// whitespace — every macOS `~/Library/Application Support/…` path — is split
// into words. Quote only then, so the Linux value stays the bare path.
#[cfg(unix)]
pub(super) fn shell_command_for(helper: &Path) -> String {
    let raw = helper.display().to_string();
    if raw.chars().any(char::is_whitespace) {
        format!("'{}'", raw.replace('\'', "'\\''"))
    } else {
        raw
    }
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), MdmError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(io_error("chmod", path))?;
    if fs::metadata(path)
        .map_err(io_error("verify chmod", path))?
        .permissions()
        .mode()
        & 0o777
        != 0o700
    {
        return Err(MdmError::HelperMode {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn prepare_helper(helper: &Path, key_path: &Path) -> Result<Vec<FileReceipt>, MdmError> {
    let receipt = write_verified(helper, &key_helper_body(key_path))?;
    set_executable(helper)?;
    Ok(vec![receipt])
}

#[cfg(target_os = "windows")]
pub(super) fn key_helper_path() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

#[cfg(target_os = "windows")]
pub(super) fn prepare_helper(
    helper: &Path,
    _key_path: &Path,
) -> Result<Vec<FileReceipt>, MdmError> {
    fs::metadata(helper).map_err(io_error("read helper executable", helper))?;
    Ok(Vec::new())
}

#[cfg(target_os = "windows")]
pub(super) fn shell_command_for(helper: &Path) -> String {
    windows_helper_command(helper)
}
