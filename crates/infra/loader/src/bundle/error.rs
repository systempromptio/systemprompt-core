//! Error surface for fetching, verifying, extracting and composing bundles.
//!
//! Every variant is a hard failure: a bundle that cannot be proven is never
//! installed. `Display` renders source names, paths and digests only — an
//! auth token or registry credential never reaches a log line through here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerifyFailure {
    #[error("archive digest is {actual}, profile pins {expected}")]
    DigestMismatch { expected: String, actual: String },

    #[error("manifest signature does not verify against any pinned key")]
    BadSignature,

    #[error("manifest is signed by unpinned key {key_id}")]
    UnknownKey { key_id: String },

    #[error("checksum mismatch for {path}")]
    FileChecksum { path: String },

    #[error("recomputed content hash does not match the manifest")]
    ContentHash,

    #[error("bundle requires core {required}, this build is {actual}")]
    RequiresCore { required: String, actual: String },

    #[error("bundle claims non-marketplace directory {dir}")]
    NotMarketplaceOnly { dir: String },

    #[error("bundle is unsigned but the profile pins ed25519 keys")]
    MissingSignature,

    #[error("unsupported signature algorithm {alg}")]
    UnsupportedAlgorithm { alg: String },

    #[error("bundle declares format {format}, this build reads {supported}")]
    UnsupportedFormat { format: u32, supported: u32 },

    #[error("extracted tree has {count} file(s) not listed in the manifest")]
    UnexpectedFiles { count: usize },

    #[error("bundle is missing bundle.json")]
    MissingManifest,
}

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("source {source_name}: fetch failed: {detail}")]
    Fetch { source_name: String, detail: String },

    #[error("source {source_name}: registry rejected the credentials")]
    Auth { source_name: String },

    #[error("verification failed: {0}")]
    Verify(#[from] VerifyFailure),

    #[error("extract failed at {path}: {detail}")]
    Extract { path: PathBuf, detail: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("policy: {detail}")]
    Policy { detail: String },

    #[error("{kind} {id} is claimed by both {first} and {second}")]
    Ownership {
        id: String,
        kind: String,
        first: String,
        second: String,
    },

    #[error("no cached bundle for source {name} and no usable fallback")]
    SourceMissing { name: String },

    #[error("bundle exceeds the {bytes} byte limit")]
    TooLarge { bytes: u64 },
}

pub type BundleResult<T> = Result<T, BundleError>;

impl BundleError {
    #[must_use]
    pub fn fetch(source_name: &str, detail: impl std::fmt::Display) -> Self {
        Self::Fetch {
            source_name: source_name.to_owned(),
            detail: detail.to_string(),
        }
    }

    #[must_use]
    pub fn policy(detail: impl std::fmt::Display) -> Self {
        Self::Policy {
            detail: detail.to_string(),
        }
    }

    #[must_use]
    pub fn extract(path: impl Into<PathBuf>, detail: impl std::fmt::Display) -> Self {
        Self::Extract {
            path: path.into(),
            detail: detail.to_string(),
        }
    }
}
