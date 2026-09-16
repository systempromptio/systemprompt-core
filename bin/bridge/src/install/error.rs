//! Install error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("installation partially completed {completed:?}; {source}")]
    Partial {
        completed: Vec<super::InstallStep>,
        #[source]
        source: Box<Self>,
    },
    #[error(transparent)]
    Config(#[from] crate::config::ConfigWriteError),
    #[error(transparent)]
    ConfigRead(#[from] crate::config::ConfigReadError),
    #[error(transparent)]
    Trust(#[from] crate::config::TrustError),
    #[error("cannot determine current executable path: {0}")]
    BinaryPath(std::io::Error),
    #[error("cannot resolve org-plugins directory for this OS")]
    OrgPluginsUnresolvable,
    #[error("directory bootstrap failed: {0}")]
    Bootstrap(#[source] std::io::Error),
    #[error(
        "permission denied creating {path} — Claude Desktop only reads org plugins from the \
         system path. Re-run as root: `sudo {bin} install --apply` (or use the install \
         script). Underlying error: {source}"
    )]
    SystemOrgPluginsDenied {
        path: std::path::PathBuf,
        bin: String,
        #[source]
        source: std::io::Error,
    },
    #[error("clean local state: {0}")]
    CleanLocalState(#[source] crate::auth::setup::SetupError),
    #[error("bridge metadata directory unresolvable")]
    MetadataUnresolvable,
    #[error("enumerate {path}: {source}")]
    Enumerate {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("remove {path}: {source}")]
    Remove {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("last-sync sentinel: {0}")]
    LastSync(#[source] crate::last_sync::ReplayStateError),
    #[error("managed settings removal failed: {0}")]
    MdmRemove(#[source] crate::install::mdm::MdmError),
    #[error("managed profile removal failed: {0}")]
    ManagedProfileRemove(String),
    #[error("credential purge failed: {0}")]
    CredentialPurge(#[source] crate::auth::setup::SetupError),
    #[error("version sentinel write failed: {0}")]
    Sentinel(std::io::Error),
    #[error("apply failed: {0}")]
    MdmApply(crate::install::mdm::MdmError),
    #[error("apply --mobileconfig failed: {0}")]
    MobileconfigApply(crate::install::mdm::MdmError),
    #[error("--apply-mobileconfig is only supported on macOS")]
    MobileconfigUnsupported,
    #[error("registering the scheduled sync job failed: {0}")]
    ScheduleApply(String),
    #[error(
        "scheduler units written ({}) but not activated: {reason}; activate them by hand or \
         re-run --apply-schedule where systemd --user is available",
        units.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
    )]
    ScheduleActivation {
        units: Vec<std::path::PathBuf>,
        reason: String,
    },
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
