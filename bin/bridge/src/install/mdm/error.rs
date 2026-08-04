//! Typed errors for the MDM policy-application surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MdmError {
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot resolve {0}")]
    Resolve(&'static str),
    #[error("{path} is not valid JSON: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path}: \"env\" is present but is not an object")]
    EnvNotObject { path: PathBuf },
    #[error(
        "gateway url {gateway} uses http:// for a non-loopback host; Bridge rejects this. Use \
         https:// or http://127.0.0.1:<port>."
    )]
    InsecureGateway { gateway: String },
    #[error("--apply on {os} must be run from a {os} binary")]
    WrongHostOs { os: &'static str },
    #[cfg(target_os = "macos")]
    #[error(
        "{source} — re-run `{binary} install --apply` and approve the authorization prompt, or \
         use `--apply-mobileconfig` for the System-Settings/MDM path."
    )]
    ApplyElevation {
        binary: &'static str,
        #[source]
        source: crate::install::elevate::ElevationError,
    },
    #[cfg(target_os = "macos")]
    #[error(transparent)]
    Elevation(#[from] crate::install::elevate::ElevationError),
    #[cfg(target_os = "windows")]
    #[error("{0}")]
    Windows(String),
}
