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

    #[error("the source pins neither a sha256 digest nor an ed25519 key")]
    NoVerification,
}

pub type BundleCause = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("source {source_name}: fetch failed: {}", describe(detail, cause.as_deref()))]
    Fetch {
        source_name: String,
        detail: String,
        #[source]
        cause: Option<BundleCause>,
    },

    #[error("source {source_name}: registry rejected the credentials")]
    Auth { source_name: String },

    #[error("verification failed: {0}")]
    Verify(#[from] VerifyFailure),

    #[error("extract failed at {path}: {}", describe(detail, cause.as_deref()))]
    Extract {
        path: PathBuf,
        detail: String,
        #[source]
        cause: Option<BundleCause>,
    },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("policy: {}", describe(detail, cause.as_deref()))]
    Policy {
        detail: String,
        #[source]
        cause: Option<BundleCause>,
    },

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

fn describe(
    detail: &str,
    cause: Option<&(dyn std::error::Error + Send + Sync + 'static)>,
) -> String {
    match cause {
        Some(cause) if detail.is_empty() => cause.to_string(),
        Some(cause) => format!("{detail}: {cause}"),
        None => detail.to_owned(),
    }
}

impl BundleError {
    #[must_use]
    pub fn fetch(source_name: &str, detail: impl Into<String>) -> Self {
        Self::Fetch {
            source_name: source_name.to_owned(),
            detail: detail.into(),
            cause: None,
        }
    }

    #[must_use]
    pub fn fetch_cause(source_name: &str, cause: impl Into<BundleCause>) -> Self {
        Self::fetch_context(source_name, String::new(), cause)
    }

    #[must_use]
    pub fn fetch_context(
        source_name: &str,
        context: impl Into<String>,
        cause: impl Into<BundleCause>,
    ) -> Self {
        Self::Fetch {
            source_name: source_name.to_owned(),
            detail: context.into(),
            cause: Some(cause.into()),
        }
    }

    #[must_use]
    pub fn policy(detail: impl Into<String>) -> Self {
        Self::Policy {
            detail: detail.into(),
            cause: None,
        }
    }

    #[must_use]
    pub fn policy_context(context: impl Into<String>, cause: impl Into<BundleCause>) -> Self {
        Self::Policy {
            detail: context.into(),
            cause: Some(cause.into()),
        }
    }

    #[must_use]
    pub fn extract(path: impl Into<PathBuf>, detail: impl Into<String>) -> Self {
        Self::Extract {
            path: path.into(),
            detail: detail.into(),
            cause: None,
        }
    }

    #[must_use]
    pub fn extract_cause(path: impl Into<PathBuf>, cause: impl Into<BundleCause>) -> Self {
        Self::Extract {
            path: path.into(),
            detail: String::new(),
            cause: Some(cause.into()),
        }
    }
}
