//! # systemprompt-ai
//!
//! Provider-agnostic LLM integration for systemprompt.io.
//!
//! `systemprompt-ai` is a [`domain`] layer crate that unifies Anthropic,
//! `OpenAI`, Gemini, and image-generation providers behind a single governed
//! pipeline with cost tracking, audit-trail persistence, and structured-output
//! validation.
//!
//! It exposes:
//!
//! - [`AiService`] — top-level orchestration of generation, tool execution,
//!   structured output, planning, and Google-Search-grounded responses.
//! - [`ImageService`] — image generation and persistence.
//! - [`ImageStorage`] / [`StorageConfig`] — local file-system blob storage for
//!   generated images.
//! - The [`AiExtension`] entrypoint that wires the crate into the
//!   [`systemprompt-extension`](systemprompt_extension) framework.
//! - Repository types ([`AiRequestRepository`], [`AiQuotaBucketRepository`], …)
//!   for persisting request audit rows, quota buckets, gateway policies, and
//!   safety findings.
//!
//! ## Error model
//!
//! All public service signatures return [`error::Result<T>`] (an alias for
//! `Result<T, AiError>`). [`AiError`](error::AiError) composes:
//!
//! - [`LlmProviderError`](systemprompt_provider_contracts::LlmProviderError)
//!   for provider-trait failures
//! - [`RepositoryError`](error::RepositoryError) for persistence
//! - common transport errors ([`reqwest::Error`], [`serde_json::Error`],
//!   [`sqlx::Error`], [`std::io::Error`], [`regex::Error`])
//! - an `Internal(String)` carve-out for cases where the upstream cause is
//!   stringified at the call site rather than typed
//!
//! The provider-trait surface ([`AiProvider`]) used over the wire bridges to
//! the boxed [`ProviderResult`](systemprompt_models::errors::ProviderResult)
//! in
//! [`services::core::ai_service`].
//!
//! ## Feature flags
//!
//! This crate has no Cargo features — all functionality is always compiled.
//! `[package.metadata.docs.rs]` sets `all-features = true` for parity with
//! sibling crates that do.
//!
//! [`domain`]: https://github.com/systempromptio/systemprompt-core/blob/main/instructions/information/architecture.md

pub mod error;
pub(crate) mod extension;
pub(crate) mod jobs;
pub mod models;
pub mod repository;
pub mod services;

pub use extension::AiExtension;

pub use services::core::{AiService, ImageService};

pub use services::storage::{ImageStorage, StorageConfig};
pub use services::tools::NoopToolProvider;
pub use systemprompt_models::ai::{GenerateResponseParams, GoogleSearchParams};

pub use systemprompt_models::ai::{
    AiMessage, AiRequest, AiRequestBuilder, AiResponse, MessageRole, ModelConfig, ModelHint,
    ModelPreferences, ProviderConfig, ResponseFormat, SamplingParams, SearchGroundedResponse,
    StructuredOutputOptions,
};

pub use systemprompt_models::ai::tools::{CallToolResult, McpTool, ToolCall, ToolExecution};

pub use systemprompt_models::services::AiConfig;

pub use models::image_generation::{
    AspectRatio, GeneratedImageRecord, ImageGenerationRequest, ImageGenerationResponse,
    ImageResolution, ReferenceImage,
};

pub use services::providers::{GeminiImageProvider, ImageProvider, ImageProviderCapabilities};

pub use repository::{
    AiGatewayPolicyRepository, AiQuotaBucketRepository, AiRequestPayloadRepository,
    AiRequestRepository, AiSafetyFindingRepository, GatewayPolicyRow, IncrementParams,
    InsertSafetyFinding, QuotaBucketDelta, QuotaBucketState, UpsertPayloadParams,
};

pub use services::tooled::ToolResultFormatter;

pub use systemprompt_models::ai::{AiProvider, DynAiProvider};
