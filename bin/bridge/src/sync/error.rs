//! Sync error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("credential cache: {0}")]
    CredentialCache(std::io::Error),
    #[error("{}", .0.one_line())]
    Partial(Box<super::SyncSummary>),
    #[error("persist sync state at {path}: {source}")]
    Persistence {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Config(#[from] crate::config::ConfigReadError),
    #[error(transparent)]
    Trust(#[from] crate::config::TrustError),
    #[error("no valid credential available; run `{bin} login` first")]
    NoCredential { bin: &'static str },
    #[error("authentication: {0}")]
    Authentication(crate::auth::ChainError),
    #[error("{0}")]
    Provision(std::io::Error),
    #[error("{0}")]
    Elevation(String),
    #[error(transparent)]
    GatewayUnauthorized(Box<CredentialRejection>),
    #[error("{0}")]
    Network(String),
    #[error(
        "manifest signature verification failed: {detail}. The manifest served by {gateway} \
         does not match the pubkey pinned from the {pin_source} — either the gateway rotated \
         its signing key, or the pin belongs to a different gateway. {fix}"
    )]
    SignatureFailed {
        detail: String,
        gateway: String,
        pin_source: &'static str,
        fix: &'static str,
    },
    #[error(
        "manifest requires schema {required} but this bridge supports up to {supported} — \
         upgrade the bridge to sync against this gateway"
    )]
    SchemaTooNew { required: u32, supported: u32 },
    #[error(
        "gateway returned a manifest this bridge cannot parse ({0}) — the gateway and bridge \
         versions are out of step; upgrade whichever is older"
    )]
    ManifestShape(String),
    #[error(
        "this bridge is {local} but the gateway requires {required} or newer — update the \
         bridge to sync against it"
    )]
    BridgeTooOld { local: String, required: String },
    #[error(
        "Cowork reads {system_path} but this process cannot write there — re-run `{bin} install \
         --apply` and approve the single administrator prompt (it provisions the Claude policy \
         and grants you write access to org-plugins), or disable the Claude Desktop host for \
         this user"
    )]
    OrgPluginsNeedElevation {
        bin: &'static str,
        system_path: String,
    },
    #[error(
        "manifest signing pubkey on file was pinned for {pinned_for}, but the gateway is now \
         {current}; explicitly pin the intended gateway key with `install \
         --apply --pubkey <base64>`"
    )]
    PubkeyStale { pinned_for: String, current: String },
    #[error("org-plugins directory not resolvable")]
    PathUnresolvable,
    #[error(
        "org-plugins directory does not exist at {path} — run `sudo {bin} install --apply` to \
         provision it (Claude Desktop only reads from this system path on macOS)"
    )]
    PathMissing { bin: &'static str, path: String },
    #[error("sync apply failed: {0}")]
    ApplyFailed(crate::host_sync::ApplyError),
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

#[derive(Debug, thiserror::Error)]
#[error(
    "gateway {gateway} rejected {credential}{identity} (HTTP {status} from {endpoint}); \
     credentials read from {config_file} (PAT: {pat_file}){override_note}. Run `{bin} login \
     <sp-live-...>` with a fresh PAT, then `{bin} whoami` to confirm."
)]
pub struct CredentialRejection {
    pub bin: &'static str,
    pub endpoint: &'static str,
    pub status: u16,
    pub gateway: String,
    pub credential: &'static str,
    pub identity: String,
    pub config_file: String,
    pub pat_file: String,
    pub override_note: String,
}

impl SyncError {
    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::CredentialCache(_)
            | Self::Partial(_)
            | Self::Persistence { .. }
            | Self::Config(_)
            | Self::Trust(_)
            | Self::Provision(_)
            | Self::Elevation(_) => ExitCode::FAILURE,
            Self::NoCredential { .. } => ExitCode::from(5),
            Self::Authentication(e) => e.exit_report().0,
            Self::GatewayUnauthorized(_) => ExitCode::from(10),
            Self::Network(_) => ExitCode::from(3),
            Self::SignatureFailed { .. } => ExitCode::from(4),
            Self::PathUnresolvable | Self::PathMissing { .. } | Self::ApplyFailed(_) => {
                ExitCode::from(1)
            },
            Self::ReplayedManifest { .. } => ExitCode::from(6),
            Self::ManifestSkew { .. } => ExitCode::from(7),
            Self::PubkeyNotPinned | Self::PubkeyStale { .. } => ExitCode::from(8),
            Self::ReplayStateCorrupt(_) => ExitCode::from(9),
            Self::SchemaTooNew { .. } | Self::ManifestShape(_) | Self::BridgeTooOld { .. } => {
                ExitCode::from(11)
            },
            Self::OrgPluginsNeedElevation { .. } => ExitCode::from(12),
        }
    }
}
