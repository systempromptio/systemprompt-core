//! Vendor-agnostic file storage for systemprompt.io.
//!
//! Every writer of user-visible files (uploads, generated images) goes
//! through the [`FileStorage`] trait so the backend can be swapped without
//! touching the domain crates. [`LocalFileStorage`] writes under one root
//! directory, which may be a local disk or a shared mount visible to every
//! replica; [`GcsFileStorage`] writes objects to one Google Cloud Storage
//! bucket; [`S3FileStorage`] writes objects to one S3-compatible bucket.
//!
//! Storage ids are paths relative to the root (`files/uploads/…`). They are
//! validated on every call: absolute paths and `..` components are rejected
//! before any I/O.
//!
//! # Modules
//!
//! - [`local`] — [`LocalFileStorage`] and the id-to-path resolver.
//! - [`gcs`] — [`GcsFileStorage`], its JSON-API client and the
//!   [`GcsTokenSource`] it authenticates with.
//! - [`s3`] — [`S3FileStorage`], signed with AWS Signature Version 4.
//! - [`probe`] — [`probe_shared_mount`], the boot-time check that a
//!   `storage.shared` root really is shared between replicas.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod gcs;
pub mod local;
mod object_id;
pub mod probe;
mod remote_read;
pub mod s3;

use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_traits::FileStorage;

pub use gcs::{
    DEFAULT_GCS_ENDPOINT, DEFAULT_METADATA_TOKEN_URL, GcsError, GcsFileStorage, GcsParams,
    GcsTokenSource, MetadataServerTokens,
};
pub use local::LocalFileStorage;
pub use probe::{SharedMountReport, probe_shared_mount};
pub use s3::{DEFAULT_S3_REGION, S3Credentials, S3Error, S3FileStorage, S3Params};

/// The resolved backend [`build_file_storage`] constructs.
pub enum FileStorageBackend {
    Local {
        root: PathBuf,
    },
    Gcs {
        params: Box<GcsParams>,
        tokens: Arc<dyn GcsTokenSource>,
        http: reqwest::Client,
    },
    S3 {
        params: Box<S3Params>,
        credentials: S3Credentials,
        http: reqwest::Client,
    },
}

impl std::fmt::Debug for FileStorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local { root } => f.debug_struct("Local").field("root", root).finish(),
            Self::Gcs { params, .. } => f
                .debug_struct("Gcs")
                .field("params", params)
                .finish_non_exhaustive(),
            Self::S3 { params, .. } => f
                .debug_struct("S3")
                .field("params", params)
                .finish_non_exhaustive(),
        }
    }
}

#[must_use]
pub fn build_file_storage(backend: FileStorageBackend) -> Arc<dyn FileStorage> {
    match backend {
        FileStorageBackend::Local { root } => Arc::new(LocalFileStorage::new(root)),
        FileStorageBackend::Gcs {
            params,
            tokens,
            http,
        } => Arc::new(GcsFileStorage::new(*params, tokens, http)),
        FileStorageBackend::S3 {
            params,
            credentials,
            http,
        } => Arc::new(S3FileStorage::new(*params, credentials, http)),
    }
}
