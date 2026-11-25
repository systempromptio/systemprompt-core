//! AI provider models — requests, responses, tool calls, sampling
//! parameters, execution plans, and the `AiProvider` trait shared
//! across LLM integrations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod embedding;
pub mod execution_plan;
pub mod media_types;
pub mod message_role;
pub mod models;
#[cfg(feature = "mcp")]
pub mod provider_trait;
pub mod request;
#[cfg(feature = "mcp")]
pub mod response;
pub mod response_format;
pub mod sampling;
pub mod template_resolver;
pub mod template_validation;
#[cfg(feature = "mcp")]
pub mod tool_result_formatter;
pub mod tools;

pub use embedding::{EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};
pub use execution_plan::{
    ExecutionState, PlannedToolCall, PlannedToolResult, PlanningResult, TemplateRef,
};
pub use media_types::{
    SUPPORTED_AUDIO_TYPES, SUPPORTED_IMAGE_TYPES, SUPPORTED_TEXT_TYPES, SUPPORTED_VIDEO_TYPES,
    is_supported_audio, is_supported_image, is_supported_media, is_supported_text,
    is_supported_video,
};
pub use message_role::{StoredMessageRole, UnknownStoredMessageRole};
pub use models::{ModelConfig, ToolModelConfig, ToolModelOverrides};
pub use request::{AiContentPart, AiMessage, AiRequest, AiRequestBuilder, MessageRole};
#[cfg(feature = "mcp")]
pub use response::{
    AiResponse, SafetyRating, SearchGroundedResponse, StreamChunk, UrlMetadata, WebSource,
};
pub use response_format::{ResponseFormat, StructuredOutputOptions};
pub use sampling::{
    ModelHint, ModelPreferences, ProviderConfig, ReasoningEffort, ReasoningParams, SamplingParams,
};
pub use template_resolver::TemplateResolver;
pub use template_validation::{PlanValidationError, TemplateValidator, ValidationErrorKind};
#[cfg(feature = "mcp")]
pub use tools::CallToolResult;
pub use tools::{McpTool, ToolCall};

#[cfg(feature = "mcp")]
pub use provider_trait::{AiProvider, DynAiProvider, GenerateResponseParams, GoogleSearchParams};
#[cfg(feature = "mcp")]
pub use tool_result_formatter::ToolResultFormatter;
