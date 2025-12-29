//! Unit tests for systemprompt-core-files crate
//!
//! Tests cover:
//! - FilesConfig (path resolution, URL generation, validation)
//! - File model (id conversion, metadata deserialization)
//! - ContentFile model (FileRole parsing, display)
//! - FileMetadata and type-specific metadata (builders, serialization)
//! - ImageMetadata and ImageGenerationInfo (builders, serialization)
//! - FileIngestionJob (mime type detection)
//! - InsertFileRequest (builder pattern)

#[cfg(test)]
mod config;

#[cfg(test)]
mod models;

#[cfg(test)]
mod jobs;

#[cfg(test)]
mod repository;
