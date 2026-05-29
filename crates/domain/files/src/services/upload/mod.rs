//! File upload pipeline: validation, storage, and persistence.
//!
//! Exposes [`FileUploadService`] together with its [`FileUploadRequest`]
//! builder, the [`FileValidator`] / [`FileCategory`] type-policy layer, the
//! [`UploadedFile`] result, and the [`FileUploadError`] /
//! [`FileValidationError`] error types.

mod error;
mod request;
mod service;
mod validator;

pub use error::FileUploadError;
pub use request::{FileUploadRequest, FileUploadRequestBuilder, UploadedFile};
pub use service::FileUploadService;
pub use validator::{FileCategory, FileValidationError, FileValidator};
