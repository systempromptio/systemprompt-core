//! Data types for stored files and their metadata.
//!
//! Re-exports the [`File`] row type and the structured-metadata family
//! ([`FileMetadata`], [`TypeSpecificMetadata`], and the per-kind detail
//! structs).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod file;
mod metadata;
mod rows;

pub use file::File;
pub use metadata::{
    AudioMetadata, DocumentMetadata, FileChecksums, FileMetadata, TypeSpecificMetadata,
    VideoMetadata,
};
pub(crate) use rows::FileRow;
pub use systemprompt_traits::{ImageGenerationInfo, ImageMetadata};
