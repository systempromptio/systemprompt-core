//! Data types for stored files and their metadata.
//!
//! Re-exports the [`File`] row type, the content-association types
//! [`ContentFile`] and [`FileRole`], and the structured-metadata family
//! ([`FileMetadata`], [`TypeSpecificMetadata`], and the per-kind detail
//! structs).

mod content_file;
mod file;
mod image_metadata;
mod metadata;

pub use content_file::{ContentFile, FileRole};
pub use file::File;
pub use image_metadata::{ImageGenerationInfo, ImageMetadata};
pub use metadata::{
    AudioMetadata, DocumentMetadata, FileChecksums, FileMetadata, TypeSpecificMetadata,
    VideoMetadata,
};
