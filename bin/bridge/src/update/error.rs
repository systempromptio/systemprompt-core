//! Self-update error taxonomy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use crate::gateway::GatewayError;

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("update check failed: {0}")]
    Check(#[from] GatewayError),
    #[error("this platform has no published build")]
    UnsupportedPlatform,
    #[error("nothing staged: {0}")]
    OutsideWindow(#[from] crate::instance::window::CandidateRefusal),
    #[error("release window: {0}")]
    Window(#[source] Box<crate::config::gateways::ProfileError>),
    #[error("gateway advertised an unparseable version {version:?}: {source}")]
    BadRemoteVersion {
        version: String,
        source: semver::Error,
    },
    #[error("this build carries an unparseable version {version:?}: {source}")]
    BadLocalVersion {
        version: String,
        source: semver::Error,
    },
    #[error("download failed: {0}")]
    Download(Box<reqwest::Error>),
    #[error("gateway rejected the download: status={status}: {rejection}")]
    DownloadStatus {
        status: reqwest::StatusCode,
        rejection: Box<crate::gateway::GatewayRejection>,
    },
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("could not resolve the update download directory")]
    NoUpdateDir,
    #[error("{path} is not writable; {hint}")]
    NotWritable { path: PathBuf, hint: String },
    #[error("could not locate the running {what}: {detail}")]
    LocateInstall { what: &'static str, detail: String },
    #[error("could not locate the running executable: {0}")]
    LocateExe(#[source] std::io::Error),
    #[error("{stage} failed: could not run {tool}: {source}")]
    Tool {
        stage: &'static str,
        tool: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "{failure}; rollback cannot remove {}: {source}; previous app remains at {}",
        target.display(),
        backup.display()
    )]
    RollbackRemove {
        failure: Box<Self>,
        target: PathBuf,
        backup: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{failure}; rollback failed: {source}; previous app remains at {}", backup.display())]
    RollbackRestore {
        failure: Box<Self>,
        backup: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("signature verification failed: {0}")]
    Signature(String),
    #[error("unpacking the download failed: {0}")]
    Unpack(String),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("relaunch failed: {0}")]
    Relaunch(std::io::Error),
    #[error("the published version changed from {expected} to {actual} mid-install; try again")]
    VersionChanged { expected: String, actual: String },
    #[error(
        "{} could not replace its program file in {}; quit {} and run the installer again",
        crate::brand::brand().app_name,
        dir.display(),
        crate::brand::brand().app_name
    )]
    ReplaceBlocked {
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the staged-update record could not be encoded: {0}")]
    StagedRecordEncode(#[source] serde_json::Error),
    #[error("the install task ended before reporting an outcome")]
    InstallTaskEnded,
    #[error("the staged-update record at {} did not read back as written", path.display())]
    StagedRecordUnverified { path: PathBuf },
    #[error("{version} is installed at {} but could not be recorded: {source}", installed.display())]
    InstalledUnrecorded {
        version: String,
        installed: PathBuf,
        #[source]
        source: Box<Self>,
    },
}

impl UpdateError {
    #[must_use]
    pub fn user_message(&self) -> String {
        let app = crate::brand::brand().app_name;
        match self {
            Self::Check(_) => {
                "The update server could not be reached. Check your connection and try again."
                    .to_owned()
            },
            Self::Download(_) | Self::DownloadStatus { .. } => {
                "The update could not be downloaded. Check your connection and try again."
                    .to_owned()
            },
            Self::ChecksumMismatch { .. } | Self::Signature(_) | Self::Unpack(_) => {
                "The downloaded update failed verification and was not installed. Try again later."
                    .to_owned()
            },
            Self::NotWritable { path, hint } => {
                format!("{app} cannot update itself at {}; {hint}.", path.display())
            },
            Self::ReplaceBlocked { .. } | Self::VersionChanged { .. } | Self::OutsideWindow(_) => {
                self.to_string()
            },
            Self::RollbackRemove { backup, .. } | Self::RollbackRestore { backup, .. } => format!(
                "The update could not be installed and the previous version could not be put \
                 back automatically. Quit {app} and run the installer again; the previous \
                 version is at {}.",
                backup.display()
            ),
            Self::Io { path, .. } => format!(
                "The update could not be written at {}. Check free disk space and permissions, \
                 then try again.",
                path.parent().unwrap_or(path).display()
            ),
            Self::InstalledUnrecorded { version, .. } => format!(
                "{app} {version} is installed. Restart {app} to finish; until then it may be \
                 offered again."
            ),
            Self::InstallTaskEnded => format!(
                "The update stopped before it finished. Try again from the {app} tray or account \
                 menu."
            ),
            Self::Relaunch(_) => format!(
                "The update is installed but {app} could not start it. Quit {app} and open it \
                 again."
            ),
            Self::UnsupportedPlatform
            | Self::Window(_)
            | Self::BadRemoteVersion { .. }
            | Self::BadLocalVersion { .. }
            | Self::NoUpdateDir
            | Self::LocateInstall { .. }
            | Self::LocateExe(_)
            | Self::Tool { .. }
            | Self::StagedRecordEncode(_)
            | Self::StagedRecordUnverified { .. } => format!(
                "{app} could not update itself. Download the latest version from your \
                 administrator and run the installer."
            ),
        }
    }

    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
