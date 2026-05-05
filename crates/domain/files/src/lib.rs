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
//! - **Jobs** — [`FileIngestionJob`] that scans the storage root and reconciles
//!   on-disk image files with database rows.
//!
//! ## Feature flags
//!
//! | Feature | Default | Effect |
//! |---------|---------|--------|
//! | _none_  | n/a     | The crate currently exposes a single feature surface; all modules are compiled unconditionally. The `[package.metadata.docs.rs] all-features = true` setting is retained so future feature additions automatically appear in published docs. |
//!
//! ## Layering
//!
//! `systemprompt-files` is a **domain** crate. It depends downward on
//! `systemprompt-database`, `systemprompt-cloud`, `systemprompt-config`,
//! `systemprompt-models`, `systemprompt-traits`, and
//! `systemprompt-provider-contracts`.

pub(crate) mod config;
pub mod error;
pub(crate) mod extension;
pub(crate) mod jobs;
pub(crate) mod models;
pub(crate) mod repository;
pub(crate) mod services;

pub use error::{FilesError, FilesResult};
pub use extension::FilesExtension;

pub use config::{
    AllowedFileTypes, FilePersistenceMode, FileUploadConfig, FilesConfig, FilesConfigValidator,
    FilesConfigYaml,
};
pub use jobs::FileIngestionJob;
pub use models::{
    AudioMetadata, ContentFile, DocumentMetadata, File, FileChecksums, FileMetadata, FileRole,
    ImageGenerationInfo, ImageMetadata, TypeSpecificMetadata, VideoMetadata,
};
pub use repository::{FileRepository, FileStats, InsertFileRequest};
pub use services::{
    FileCategory, FileUploadError, FileUploadRequest, FileUploadRequestBuilder, FileUploadService,
    FileValidationError, FileValidator, FilesAiPersistenceProvider, UploadedFile,
};
