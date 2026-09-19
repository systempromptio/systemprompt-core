//! Internal, elevated (UAC) worker for the machine-wide install steps: the
//! HKLM Claude Desktop managed-policy write and org-plugins provisioning.
//!
//! Hidden from `--help`; the GUI re-launches the bridge with this subcommand
//! and a staged JSON job file when a non-elevated install hits the
//! ACL-protected `SOFTWARE\Policies\Claude` subtree or the admin-write-only
//! org-plugins directory, and reads the outcome from the result file argument.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

#[cfg(target_os = "windows")]
pub(crate) fn cmd(args: &[String]) -> ExitCode {
    let (Some(job_path), Some(result_path)) = (args.get(2), args.get(3)) else {
        crate::stdio::diag("usage: __install-claude-policy <job-path> <result-path>");
        return ExitCode::from(2);
    };
    crate::install::elevated_job::perform_elevated_write(job_path, result_path)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn cmd(_args: &[String]) -> ExitCode {
    crate::stdio::diag("__install-claude-policy is supported only on Windows");
    ExitCode::FAILURE
}

// Why: the Task Scheduler action for the SYSTEM policy writer; the spool root
// is the task's fixed argument and only has to agree with the machine's own.
#[cfg(target_os = "windows")]
pub(crate) fn cmd_apply_policy_task(args: &[String]) -> ExitCode {
    let Some(spool_root) = args.get(2) else {
        crate::stdio::diag("usage: __apply-policy-task <spool-root>");
        return ExitCode::from(2);
    };
    crate::install::policy_writer::perform_task(spool_root)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn cmd_apply_policy_task(_args: &[String]) -> ExitCode {
    crate::stdio::diag("__apply-policy-task is supported only on Windows");
    ExitCode::FAILURE
}
