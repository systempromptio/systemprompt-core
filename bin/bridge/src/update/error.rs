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
    #[error("gateway rejected the download: status={status}")]
    DownloadStatus { status: reqwest::StatusCode },
    /// The downloaded bytes did not match the manifest digest. Treated as
    /// hostile, never as a transient error to retry around.
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("could not resolve the staging directory")]
    NoStagingDir,
    /// The installed binary or bundle is somewhere this process may not write —
    /// a per-machine install, or a macOS app still running from the mounted
    /// dmg.
    #[error("{path} is not writable; {hint}")]
    NotWritable { path: PathBuf, hint: String },
    #[error("could not locate the running {what}: {detail}")]
    LocateInstall { what: &'static str, detail: String },
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
    /// A release was published between the check and the install, so the bytes
    /// on offer are no longer the ones the user agreed to.
    #[error("the published version changed from {expected} to {actual} mid-install; try again")]
    VersionChanged { expected: String, actual: String },
}

impl UpdateError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
