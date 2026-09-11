//! Windows CLI credential helper invocation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

#[must_use]
pub fn windows_helper_command(executable: &Path) -> String {
    let path = executable.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference = 'Stop'; & '{path}' credential-helper --host claude-code; exit $LASTEXITCODE"
    );
    // Why: -EncodedCommand accepts UTF-16LE and avoids expansion by the shell
    // Claude Code uses to launch helpers, including Git Bash and cmd.exe.
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    format!(
        "powershell.exe -NoProfile -NonInteractive -EncodedCommand {}",
        STANDARD.encode(bytes)
    )
}
