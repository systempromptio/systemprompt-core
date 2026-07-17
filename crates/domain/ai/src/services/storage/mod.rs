//! Local file-system blob storage for generated images. See
//! [`crate::ImageStorage`] / [`crate::StorageConfig`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod image_storage;

pub use image_storage::{ImageStorage, StorageConfig};
