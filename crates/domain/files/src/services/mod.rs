//! Application services for the files domain.
//!
//! Exposes the [`FileUploadService`] upload pipeline with its
//! request/validation surface, [`FileDeleteService`], which removes a file's
//! stored object together with its row, and [`FilesAiPersistenceProvider`],
//! which persists AI-generated files through the provider framework.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_provider;
mod delete;
mod upload;

pub use ai_provider::FilesAiPersistenceProvider;
pub use delete::{FileDeleteError, FileDeleteService, StoredObjectOutcome};
pub use upload::{
    FileCategory, FileUploadError, FileUploadRequest, FileUploadRequestBuilder, FileUploadService,
    FileValidationError, FileValidator, UploadedFile,
};
