//! File upload pipeline: validation, storage, and persistence.
//!
//! Exposes [`FileUploadService`] together with its [`FileUploadRequest`]
//! builder, the [`FileValidator`] / [`FileCategory`] type-policy layer, the
//! [`UploadedFile`] result, and the [`FileUploadError`] /
//! [`FileValidationError`] error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod request;
mod service;
mod validator;

pub use error::FileUploadError;
pub use request::{FileUploadRequest, FileUploadRequestBuilder, UploadedFile};
pub use service::FileUploadService;
pub use validator::{FileCategory, FileValidationError, FileValidator};
