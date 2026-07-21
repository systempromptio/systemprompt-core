//! Install error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("cannot determine current executable path: {0}")]
    BinaryPath(std::io::Error),
    #[error("cannot resolve org-plugins directory for this OS")]
    OrgPluginsUnresolvable,
    #[error("{0}")]
    Bootstrap(String),
    #[error("version sentinel write failed: {0}")]
    Sentinel(std::io::Error),
    #[error("apply failed: {0}")]
    MdmApply(String),
    #[error("apply --mobileconfig failed: {0}")]
    MobileconfigApply(String),
    #[error("--apply-mobileconfig is only supported on macOS")]
    MobileconfigUnsupported,
    #[error("registering the scheduled sync job failed: {0}")]
    ScheduleApply(String),
    #[error("--apply-schedule can only register a job for the OS it runs on")]
    ScheduleOsMismatch,
    #[error("failed to write {path}: {source}")]
    Schedule {
        path: String,
        source: std::io::Error,
    },
}

impl InstallError {
    pub const EXIT_CODE: ExitCode = ExitCode::FAILURE;
}
