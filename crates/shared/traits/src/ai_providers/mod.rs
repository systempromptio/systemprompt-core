//! AI generation file storage and session provider traits.
//!
//! Re-exports the typed [`AiProviderError`], the
//! [`AiFilePersistenceProvider`] trait for storing AI-generated files, the
//! [`AiSessionProvider`] trait for AI session lifecycle, the
//! [`AiRequestTrace`] read seam over the request trace, and the
//! [`ImageMetadata`] / [`ImageGenerationInfo`] value types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod files;
mod image;
mod sessions;
mod trace;

pub use error::{AiProviderError, AiProviderResult};
pub use files::{
    AiFilePersistenceProvider, AiGeneratedFile, DynAiFilePersistenceProvider, ImageStorageConfig,
    InsertAiFileParams,
};
pub use image::{ImageGenerationInfo, ImageMetadata};
pub use sessions::{AiSessionProvider, CreateAiSessionParams, DynAiSessionProvider};
pub use trace::{
    AiRequestTrace, DynAiRequestTrace, TraceMessage, TraceRequestStatus, TraceRequestUsage,
    TraceSample, TraceSampleFilter, TraceSampleMode,
};
