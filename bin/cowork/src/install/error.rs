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
    #[error("failed to write {path}: {source}")]
    Schedule {
        path: String,
        source: std::io::Error,
    },
}

impl InstallError {
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::from(1)
    }
}
