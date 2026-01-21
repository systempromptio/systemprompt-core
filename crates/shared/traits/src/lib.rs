pub mod ai_providers;
pub mod analytics;
pub mod artifact;
pub mod auth;
pub mod content;
pub mod context;
pub mod context_provider;
pub mod domain_config;
pub mod events;
pub mod extension_error;
pub mod file_upload;
pub mod jwt;
pub mod log_service;
pub mod mcp_service;
pub mod module;
pub mod process;
pub mod registry;
pub mod repository;
pub mod scheduler;
pub mod service;
pub mod session_analytics;
pub mod storage;
pub mod validation;
pub mod validation_report;

pub use systemprompt_provider_contracts::{
    submit_job, ChatMessage, ChatRequest, ChatResponse, ChatRole, ChatStream, Job, JobContext,
    JobResult, LlmProvider, LlmProviderError, LlmProviderResult, SamplingParameters, TokenUsage,
    ToolCallRequest, ToolCallResult, ToolContent, ToolContext, ToolDefinition,
    ToolExecutionContext, ToolExecutor, ToolProvider, ToolProviderError, ToolProviderResult,
};

pub use context::{
    AppContext, ConfigProvider, ContextPropagation, DatabaseHandle, InjectContextHeaders, Module,
    ModuleRegistry,
};

#[cfg(feature = "axum")]
pub use context::ApiModule;

pub use systemprompt_identifiers::{
    parse_database_datetime, DbValue, FromDbValue, JsonRow, ToDbValue,
};

pub use repository::{CrudRepository, Repository, RepositoryError};

pub use service::{AsyncService, Service};

pub use log_service::LogService;

pub use context_provider::{
    ContextProvider, ContextProviderError, ContextWithStats, DynContextProvider,
};

pub use artifact::{schemas, ArtifactSupport};

pub use validation::{MetadataValidation, Validate, ValidationError, ValidationResult};

pub use events::{
    AnalyticsEvent, AnalyticsEventPublisher, LogEventData, LogEventLevel, LogEventPublisher,
    UserEvent, UserEventPublisher,
};

pub use analytics::{
    AnalyticsProvider, AnalyticsProviderError, AnalyticsResult, AnalyticsSession,
    CreateSessionInput, DynAnalyticsProvider, DynFingerprintProvider, FingerprintProvider,
    SessionAnalytics,
};

pub use auth::{
    AuthAction, AuthPermission, AuthProvider, AuthProviderError, AuthResult, AuthUser,
    AuthorizationProvider, DynAuthProvider, DynAuthorizationProvider, DynRoleProvider,
    DynUserProvider, RoleProvider, TokenClaims, TokenPair, UserProvider,
};

pub use storage::{FileStorage, StoredFileId, StoredFileMetadata};

pub use ai_providers::{
    AiFilePersistenceProvider, AiGeneratedFile, AiProviderError, AiProviderResult,
    AiSessionProvider, CreateAiSessionParams, DynAiFilePersistenceProvider, DynAiSessionProvider,
    ImageGenerationInfo, ImageMetadata, ImageStorageConfig, InsertAiFileParams,
};

pub use scheduler::JobStatus;

pub use registry::{
    AgentInfo, AgentRegistryProvider, DynAgentRegistryProvider, DynMcpRegistryProvider,
    McpRegistryProvider, McpServerInfo, RegistryError, ServiceOAuthConfig,
};

pub use extension_error::{ApiError, ExtensionError, McpErrorData};

pub use domain_config::{DomainConfig, DomainConfigError, DomainConfigRegistry};

pub use validation_report::{
    StartupValidationError, StartupValidationReport, ValidationReport, ValidationWarning,
};

pub use file_upload::{
    DynFileUploadProvider, FileUploadInput, FileUploadProvider, FileUploadProviderError,
    FileUploadResult, UploadedFileInfo,
};

pub use jwt::{
    AgentJwtClaims, DynJwtValidationProvider, GenerateTokenParams, JwtProviderError, JwtResult,
    JwtValidationProvider,
};

pub use mcp_service::{
    DynMcpServiceProvider, McpServerMetadata, McpServiceProvider, McpServiceProviderError,
    McpServiceResult,
};

pub use process::{
    DynProcessCleanupProvider, ProcessCleanupProvider, ProcessProviderError, ProcessResult,
};

pub use session_analytics::{
    DynSessionAnalyticsProvider, SessionAnalyticsProvider, SessionAnalyticsProviderError,
    SessionAnalyticsResult,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

mod startup_events;
pub use startup_events::*;
