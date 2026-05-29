//! Application services for the files domain.
//!
//! Exposes the [`FileUploadService`] upload pipeline with its
//! request/validation surface, and [`FilesAiPersistenceProvider`], which
//! persists AI-generated files through the provider framework.

mod ai_provider;
mod providers;
mod upload;

pub use ai_provider::FilesAiPersistenceProvider;
pub use upload::{
    FileCategory, FileUploadError, FileUploadRequest, FileUploadRequestBuilder, FileUploadService,
    FileValidationError, FileValidator, UploadedFile,
};
