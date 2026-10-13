//! The `apiKeyHelper` the bridge hands Claude Code: a `sh` script that execs
//! the bridge's credential helper on Unix, the bridge executable itself on
//! Windows, and the shell command line each is invoked with.
//!
//! Both platforms present the `claude-code` host token, never the loopback
//! secret, so the proxy can attribute the traffic to Claude Code.
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
        "powershell.exe -NoProfile -NonInteractive{ENCODED_COMMAND}{}",
        STANDARD.encode(bytes)
    )
}

const ENCODED_COMMAND: &str = " -EncodedCommand ";

// Why: Claude Code's settings carry the Windows helper base64-encoded, so a
// helper naming an executable a reinstall moved is recognisable as ours only
// by decoding it.
#[must_use]
pub fn is_bridge_windows_helper(command: &str) -> bool {
    let Some((prefix, encoded)) = command.split_once(ENCODED_COMMAND) else {
        return false;
    };
    if !prefix.starts_with("powershell.exe") {
        return false;
    }
    let Ok(bytes) = STANDARD.decode(encoded.trim()) else {
        return false;
    };
    if bytes.len() % 2 != 0 {
        return false;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units).is_ok_and(|script| {
        script.ends_with("credential-helper --host claude-code; exit $LASTEXITCODE")
    })
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
fn key_helper_body(executable: &Path) -> String {
    let bin = crate::brand::brand().binary_name;
    format!(
        "#!/bin/sh\n\
         # Written by `{bin} install --apply`. Rewritten on every apply — do not edit.\n\
         exec '{exe}' credential-helper --host claude-code\n",
        exe = executable.display().to_string().replace('\'', "'\\''"),
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
pub(super) fn prepare_helper(helper: &Path) -> Result<Vec<FileReceipt>, MdmError> {
    let executable =
        std::env::current_exe().map_err(io_error("resolve the bridge executable", helper))?;
    let receipt = write_verified(helper, &key_helper_body(&executable))?;
    set_executable(helper)?;
    Ok(vec![receipt])
}

#[cfg(target_os = "windows")]
pub(super) fn key_helper_path() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

#[cfg(target_os = "windows")]
pub(super) fn prepare_helper(helper: &Path) -> Result<Vec<FileReceipt>, MdmError> {
    fs::metadata(helper).map_err(io_error("read helper executable", helper))?;
    Ok(Vec::new())
}

#[cfg(target_os = "windows")]
pub(super) fn shell_command_for(helper: &Path) -> String {
    windows_helper_command(helper)
}
