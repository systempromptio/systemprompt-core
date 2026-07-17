//! AI generation file storage and session provider traits.
//!
//! Re-exports the typed [`AiProviderError`], the
//! [`AiFilePersistenceProvider`] trait for storing AI-generated files, the
//! [`AiSessionProvider`] trait for AI session lifecycle, and the
//! [`ImageMetadata`] / [`ImageGenerationInfo`] value types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod files;
mod image;
mod sessions;

pub use error::{AiProviderError, AiProviderResult};
pub use files::{
    AiFilePersistenceProvider, AiGeneratedFile, DynAiFilePersistenceProvider, ImageStorageConfig,
    InsertAiFileParams,
};
pub use image::{ImageGenerationInfo, ImageMetadata};
pub use sessions::{AiSessionProvider, CreateAiSessionParams, DynAiSessionProvider};
