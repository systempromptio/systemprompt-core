//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("no valid credential available; run `{bin} login` first")]
    NoCredential { bin: &'static str },
    #[error(
        "gateway rejected credentials ({endpoint}, HTTP {status}). The cached token is invalid or \
         revoked. Run `{bin} login <sp-live-...>` with a fresh PAT, then `{bin} whoami` to confirm."
    )]
    GatewayUnauthorized {
        bin: &'static str,
        endpoint: &'static str,
        status: u16,
    },
    #[error("{0}")]
    Network(String),
    #[error("manifest signature verification failed: {0}")]
    SignatureFailed(String),
    #[error("org-plugins directory not resolvable")]
    PathUnresolvable,
    #[error(
        "org-plugins directory does not exist at {path} — run `sudo {bin} install --apply` to \
         provision it (Claude Desktop only reads from this system path on macOS)"
    )]
    PathMissing { bin: &'static str, path: String },
    #[error("sync apply failed: {0}")]
    ApplyFailed(crate::sync::apply::ApplyError),
    #[error("manifest replay rejected: incoming {incoming} is not newer than last applied {last}")]
    ReplayedManifest { last: String, incoming: String },
    #[error("manifest clock skew rejected: not_before {not_before} outside +/- 5m of now {now}")]
    ManifestSkew { not_before: String, now: String },
    #[error(
        "manifest signing pubkey is not pinned; provide it out of band via MDM (`install --apply \
         --pubkey <base64>`) or rerun with `--allow-tofu` to fetch over the wire (insecure \
         first-run)"
    )]
    PubkeyNotPinned,
    #[error("replay state corrupt: {0}")]
    ReplayStateCorrupt(#[from] crate::sync::replay::ReplayStateError),
}

impl SyncError {
    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::NoCredential { .. } => ExitCode::from(5),
            Self::GatewayUnauthorized { .. } => ExitCode::from(10),
            Self::Network(_) => ExitCode::from(3),
            Self::SignatureFailed(_) => ExitCode::from(4),
            Self::PathUnresolvable | Self::PathMissing { .. } | Self::ApplyFailed(_) => {
                ExitCode::from(1)
            },
            Self::ReplayedManifest { .. } => ExitCode::from(6),
            Self::ManifestSkew { .. } => ExitCode::from(7),
            Self::PubkeyNotPinned => ExitCode::from(8),
            Self::ReplayStateCorrupt(_) => ExitCode::from(9),
        }
    }
}
