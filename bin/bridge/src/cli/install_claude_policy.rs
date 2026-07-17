//! Internal, elevated (UAC) worker for the machine-wide (HKLM) Claude Desktop
//! managed-policy write.
//!
//! Hidden from `--help`; the GUI re-launches the bridge with this subcommand
//! when a non-elevated install hits the ACL-protected
//! `SOFTWARE\Policies\Claude` subtree, and reads the outcome from the result
//! file argument.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

#[cfg(target_os = "windows")]
pub(crate) fn cmd(args: &[String]) -> ExitCode {
    let (Some(reg_path), Some(result_path)) = (args.get(2), args.get(3)) else {
        crate::obs::output::diag("usage: __install-claude-policy <reg-path> <result-path>");
        return ExitCode::from(2);
    };
    crate::integration::claude_desktop::elevate::perform_elevated_write(reg_path, result_path)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn cmd(_args: &[String]) -> ExitCode {
    crate::obs::output::diag("__install-claude-policy is supported only on Windows");
    ExitCode::FAILURE
}
