pub mod error;
pub mod extension;
pub mod jobs;
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

pub use repository::{AiRequestRepository, CreateAiRequest};

pub use services::tooled::ToolResultFormatter;

pub use systemprompt_models::ai::{AiProvider, DynAiProvider};
