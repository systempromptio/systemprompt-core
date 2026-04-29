use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("no valid credential available; run `systemprompt-cowork login` first")]
    NoCredential,
    #[error("{0}")]
    Network(String),
    #[error("manifest signature verification failed: {0}")]
    SignatureFailed(String),
    #[error("org-plugins directory not resolvable")]
    PathUnresolvable,
    #[error(
        "org-plugins directory does not exist at {path} — run `sudo systemprompt-cowork install \
         --apply` to provision it (Claude Desktop only reads from this system path on macOS)"
    )]
    PathMissing { path: String },
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
}

impl SyncError {
    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        match self {
            SyncError::NoCredential => ExitCode::from(5),
            SyncError::Network(_) => ExitCode::from(3),
            SyncError::SignatureFailed(_) => ExitCode::from(4),
            SyncError::PathUnresolvable
            | SyncError::PathMissing { .. }
            | SyncError::ApplyFailed(_) => ExitCode::from(1),
            SyncError::ReplayedManifest { .. } => ExitCode::from(6),
            SyncError::ManifestSkew { .. } => ExitCode::from(7),
            SyncError::PubkeyNotPinned => ExitCode::from(8),
        }
    }
}
