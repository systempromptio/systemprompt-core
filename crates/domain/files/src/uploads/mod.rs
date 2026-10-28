//! Resumable, content-checked uploads that end in a document blob.
//!
//! A client opens a session declaring the file's name, MIME type, size and
//! SHA-256 ([`UploadService::create`]), stores fixed-size parts in any
//! order and retries any of them ([`UploadService::put_part`]), then
//! completes it ([`UploadService::complete`]): the parts are streamed into
//! one object, the hash is verified, the content checks run and the
//! result is registered as a content-addressed, deduplicated blob in
//! `document_blobs`. The blob's hash is the `blob_ref` an import names.
//!
//! Sessions are bounded per user by the `files.documents.uploads` limits
//! ([`UploadLimits`](crate::UploadLimits)); [`job`] closes the ones left
//! open past their lifetime. An unimported blob is collected by the
//! document blob GC after its grace period, like any unreferenced blob.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod blobs;
mod claims;
mod complete;
pub mod error;
mod ingest;
pub mod job;
mod lifecycle;
pub mod models;
mod parts;
pub mod repository;
mod safety;
mod service;
mod stream;

pub use error::{RefusalReason, UploadError, UploadResult};
pub use ingest::Ingested;
pub use job::DocumentUploadExpiryJob;
pub use models::{
    Allowance, BlobOrigin, CompletedUpload, NewUpload, UploadPart, UploadProgress, UploadSession,
    UploadStatus,
};
pub use repository::DocumentUploadRepository;
pub use service::{UploadService, UploadServiceParts};
