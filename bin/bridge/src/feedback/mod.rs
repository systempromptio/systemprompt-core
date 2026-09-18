//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod credentials;
pub mod enrol;
pub mod hooks;
pub mod opencode_session;
pub mod outbox;
pub mod readback;
pub mod sessions;
mod sync;
pub mod transport;

pub use sync::{
    RecoveryProgress, deliver, recover_current_manifest, recover_manifest_installations,
    recover_pending, retry_pending,
};

#[derive(Debug, thiserror::Error)]
pub enum FeedbackError {
    #[error("feedback storage failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("feedback data is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("device enrollment is required")]
    EnrollmentRequired,
    #[error("feedback belongs to a different gateway or device")]
    Scope,
    #[error("feedback outbox is full; installation remains unacknowledged")]
    Full,
    #[error("host installation is unavailable for readback")]
    HostUnavailable,
    #[error("installation readback mismatch or unsafe path")]
    Readback,
    #[error("feedback transport is unavailable")]
    Transport,
    #[error("feedback transport failed: {0}")]
    Http(#[source] reqwest::Error),
    #[error("feedback operation timed out: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),
    #[error("bridge configuration is unreadable: {0}")]
    Config(#[source] ConfigurationFailure),
    #[error("feedback gateway url is invalid: {0}")]
    InvalidGateway(#[from] url::ParseError),
    #[error("feedback header value is invalid: {0}")]
    Header(#[from] http::header::InvalidHeaderValue),
    #[error("feedback contract violation: {0}")]
    Contract(#[from] systemprompt_models::feedback::FeedbackContractError),
    #[error("feedback request rejected with status {0}")]
    Rejected(u16),
    #[error("device enrolment failed: {0}")]
    Gateway(#[from] crate::gateway::errors::GatewayError),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationFailure {
    #[error("configuration path is unavailable")]
    PathUnavailable,
    #[error("configuration could not be read ({0:?})")]
    Read(std::io::ErrorKind),
    #[error("configuration syntax is invalid at byte range {0:?}")]
    Malformed(Option<std::ops::Range<usize>>),
}

impl From<crate::config::ConfigReadError> for FeedbackError {
    fn from(error: crate::config::ConfigReadError) -> Self {
        let safe = match error {
            crate::config::ConfigReadError::PathUnresolvable => {
                ConfigurationFailure::PathUnavailable
            },
            crate::config::ConfigReadError::Read { source, .. } => {
                ConfigurationFailure::Read(source.kind())
            },
            crate::config::ConfigReadError::Malformed { source, .. } => {
                ConfigurationFailure::Malformed(source.span())
            },
        };
        Self::Config(safe)
    }
}

impl From<reqwest::Error> for FeedbackError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error.without_url())
    }
}

pub type Result<T> = std::result::Result<T, FeedbackError>;

// Why: the manifest names hosts in the gateway's vocabulary and the bridge in
// its own; `EvaluatorClient::accepts_host_name` is the one place both are
// listed, so no second alias table can drift from it.
pub fn client_kind(host: &str) -> Option<systemprompt_models::feedback::EvaluatorClient> {
    systemprompt_models::wire::origin::ClientKind::ALL
        .into_iter()
        .filter_map(|kind| systemprompt_models::feedback::EvaluatorClient::try_from(kind).ok())
        .find(|client| client.accepts_host_name(host))
}

pub fn metadata_root() -> Result<std::path::PathBuf> {
    crate::config::paths::bridge_metadata_dir()
        .map(|root| root.join("feedback"))
        .ok_or(FeedbackError::EnrollmentRequired)
}

pub async fn installation_lock() -> Result<std::fs::File> {
    let root = metadata_root()?;
    tokio::task::spawn_blocking(move || -> Result<std::fs::File> {
        crate::fsutil::create_dir_all_mode_0700(&root)?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("install.lock"))?;
        file.lock()?;
        Ok(file)
    })
    .await
    .map_err(|error| FeedbackError::Io(std::io::Error::other(error)))?
}
