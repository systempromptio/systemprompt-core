//! Unit tests for systemprompt-core-files crate
//!
//! Tests cover:
//! - FilesConfig (config types, serialization, defaults)
//! - File model (id conversion, metadata deserialization)
//! - ContentFile model (FileRole parsing, display)
//! - FileMetadata and type-specific metadata (builders, serialization)
//! - ImageMetadata and ImageGenerationInfo (builders, serialization)
//! - FileIngestionJob (mime type detection)
//! - InsertFileRequest (builder pattern)
//! - FileValidator (MIME type validation, categorization, extension mapping)
//! - FileCategory (storage subdirs, display names)
//! - FileUploadRequest (builder pattern)
//! - FilesExtension (metadata, schemas, dependencies)
//! - Error types (FileValidationError, FileUploadError)

#[cfg(test)]
mod config;

#[cfg(test)]
mod extension;

#[cfg(test)]
mod models;

#[cfg(test)]
mod jobs;

#[cfg(test)]
mod repository;

#[cfg(test)]
mod services;
