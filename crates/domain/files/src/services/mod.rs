//! Application services for the files domain.
//!
//! Exposes the [`FileUploadService`] upload pipeline with its
//! request/validation surface, and [`FilesAiPersistenceProvider`], which
//! persists AI-generated files through the provider framework.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_provider;
mod upload;

pub use ai_provider::FilesAiPersistenceProvider;
pub use upload::{
    FileCategory, FileUploadError, FileUploadRequest, FileUploadRequestBuilder, FileUploadService,
    FileValidationError, FileValidator, UploadedFile,
};
