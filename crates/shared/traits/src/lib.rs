//! # systemprompt-traits
//!
//! Trait-first interface contracts for the systemprompt.io platform.
//!
//! This crate defines the abstractions every other layer (infra, domain,
//! app, entry) implements or consumes: configuration, database handle,
//! analytics, authentication, JWT, file storage, repositories, schedulers,
//! and the cross-cutting [`ExtensionError`] contract.
//!
//! ## Layering
//!
//! `systemprompt-traits` lives in the `shared` layer and depends only on
//! [`systemprompt-identifiers`](systemprompt_identifiers) and
//! [`systemprompt-provider-contracts`](systemprompt_provider_contracts).
//! Concrete implementations live in their respective domain or infra
//! crates and are wired together at the entry layer.
//!
//! ## Errors
//!
//! Each provider trait pairs with a typed `thiserror`-derived error enum
//! (e.g. [`AnalyticsProviderError`], [`AuthProviderError`],
//! [`JwtProviderError`], [`FileStorageError`],
//! [`ContextPropagationError`]). The crate also defines the cross-cutting
//! [`ExtensionError`] trait which downstream errors implement so the API
//! and MCP transports can render them uniformly.
//!
//! ## Async traits
//!
//! Most provider traits are exposed as `Arc<dyn TraitName>` (see the
//! `Dyn*` aliases). Until trait dispatch supports native `async fn` on
//! `dyn` traits, these continue to rely on `#[async_trait]`. Each trait
//! whose contract requires it is annotated with that rationale.
//!
//! ## Feature flags
//!
//! | Feature | Effect |
//! |---------|--------|
//! | `default` | No optional features. |
//! | `web`     | Enables the `ApiModule` trait and pulls in `axum` for HTTP routing. |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod ai_providers;
pub mod analytics;
pub mod auth;
pub mod content;
pub mod context;
pub mod context_provider;
pub mod domain_config;
pub mod events;
pub mod extension_error;
pub mod jwt;
pub mod log_service;
pub mod module;
pub mod registry;
pub mod repository;
pub mod scheduler;
pub mod service;
pub mod storage;
pub mod validation;
pub mod validation_report;

pub use systemprompt_provider_contracts::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, ChatStream, Job, JobContext, JobResult,
    LlmProvider, LlmProviderError, LlmProviderResult, ProviderError, ProviderResult,
    SamplingParameters, TokenUsage, ToolCallRequest, ToolCallResult, ToolContent, ToolContext,
    ToolDefinition, ToolExecutionContext, ToolExecutor, ToolProvider, ToolProviderError,
    ToolProviderResult, submit_job,
};

pub use context::{
    AppContext, ConfigProvider, ContextPropagation, ContextPropagationError,
    ContextPropagationResult, DatabaseHandle, InjectContextHeaders, Module, ModuleRegistry,
};

#[cfg(feature = "web")]
pub use context::ApiModule;

pub use systemprompt_identifiers::{
    DbValue, FromDbValue, JsonRow, ToDbValue, parse_database_datetime,
};

pub use repository::RepositoryError;

pub use service::{AsyncService, Service};

pub use log_service::LogService;

pub use context_provider::{
    ContextProvider, ContextProviderError, ContextWithStats, DynContextProvider,
};

pub use validation::{MetadataValidation, Validate, ValidationError, ValidationResult};

pub use events::{
    AnalyticsEvent, AnalyticsEventPublisher, LogEventData, LogEventLevel, LogEventPublisher,
    UserEvent, UserEventPublisher,
};

pub use analytics::{
    ActiveSession, AnalyticsProvider, AnalyticsProviderError, AnalyticsResult, AnalyticsSession,
    CreateSessionInput, DynAnalyticsProvider, DynFingerprintProvider, ExtractSignals,
    FingerprintProvider, SessionAnalytics,
};

pub use auth::{
    AuthProviderError, AuthResult, AuthUser, DynRoleProvider, DynUserProvider,
    FederatedIdentityClaims, RoleProvider, UserProvider,
};

pub use storage::{
    FileStorage, FileStorageError, FileStorageResult, StoredFileId, StoredFileMetadata,
};

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

pub use jwt::{
    AgentJwtClaims, DynJwtValidationProvider, GenerateTokenParams, JwtProviderError, JwtResult,
    JwtValidationProvider,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

mod startup_events;
pub use startup_events::*;
