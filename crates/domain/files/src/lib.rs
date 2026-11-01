//! # systemprompt-files
//!
//! File storage, metadata, and access control for the systemprompt.io AI
//! governance platform. The crate provides:
//!
//! - **Configuration** — profile-driven [`FilesConfig`] resolving storage roots
//!   and per-MIME upload policies via YAML overrides.
//! - **Models** — typed [`File`], [`FileMetadata`], [`ContentFile`] and
//!   [`FileRole`] structures backed by Postgres.
//! - **Repositories** — `sqlx`-backed persistence for file rows and
//!   content↔file associations.
//! - **Services** — upload validation, content services, AI-persistence glue.
//! - **Uploads** — resumable, content-checked uploads that end in a
//!   content-addressed document blob ([`uploads`]).
//! - **Jobs** — [`FileIngestionJob`] that scans the storage root and reconciles
//!   on-disk image files with database rows.
//!
//! ## Feature flags
//!
//! | Feature | Default | Effect |
//! |---------|---------|--------|
//! | `office` | off | The `.docx`, `.xlsx` and `.pptx` [`DocumentCodec`](systemprompt_provider_contracts::documents::DocumentCodec)s in `documents::codecs::{docx,xlsx,pptx}`, built on `zip`, `quick-xml`, `ammonia` and `umya-spreadsheet`. |
//! | `collab` | off | Live co-editing of HTML and Markdown documents in `documents::collab`, over a [`DocumentEngine`](documents::engine::DocumentEngine) the composition root selects; no CRDT crate is linked here. |
//!
//! ## Layering
//!
//! `systemprompt-files` is a **domain** crate. It depends downward on
//! `systemprompt-database`, `systemprompt-cloud`, `systemprompt-config`,
//! `systemprompt-models`, `systemprompt-traits`, and
//! `systemprompt-provider-contracts`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod config;
mod document_schemas;
pub mod documents;
pub mod error;
pub(crate) mod extension;
pub mod issues;
pub(crate) mod jobs;
pub(crate) mod models;
pub(crate) mod repository;
pub(crate) mod services;
pub mod uploads;

#[doc(hidden)]
pub use ::inventory as __inventory;
pub use error::{FilesError, FilesResult};
pub use extension::FilesExtension;

pub use config::{
    AllowedFileTypes, CollabConfig, CollabOwnerConfig, DocumentLimits, DocumentsConfig,
    FilePersistenceMode, FileUploadConfig, FilesConfig, FilesConfigValidator, FilesConfigYaml,
    IssuesConfig, MAX_DOCUMENT_UPLOAD_BYTES, MIN_DOCUMENT_UPLOAD_BYTES, UploadLimits,
};
pub use jobs::FileIngestionJob;
pub use models::{
    AudioMetadata, ContentFile, DocumentMetadata, File, FileChecksums, FileMetadata, FileRole,
    ImageGenerationInfo, ImageMetadata, TypeSpecificMetadata, VideoMetadata,
};
pub use repository::{FileRepository, FileStats, FilesOwnerReassignment, InsertFileRequest};
pub use services::{
    FileCategory, FileUploadError, FileUploadRequest, FileUploadRequestBuilder, FileUploadService,
    FileValidationError, FileValidator, FilesAiPersistenceProvider, UploadedFile,
};
