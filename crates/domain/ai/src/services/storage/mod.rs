//! Local file-system blob storage for generated images. See
//! [`crate::ImageStorage`] / [`crate::StorageConfig`].

pub mod image_storage;

pub use image_storage::{ImageStorage, StorageConfig};
