//! # systemprompt-traits
//!
//! Trait-first interface contracts for the systemprompt.io platform.
//!
//! This crate defines the abstractions every other layer (infra, domain,
//! app, entry) implements or consumes: configuration, database handle,
//! analytics, authentication, JWT, file storage, repositories, and the
//! cross-cutting [`ExtensionError`] contract. It also hosts the one
//! concrete runtime primitive every layer shares: the [`BackgroundTasks`]
//! owner for work that outlives its caller.
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
//! - `sqlx` — `From<sqlx::Error>` for [`RepositoryError`], classifying a
//!   database error by SQLSTATE. Enabled by `systemprompt-database`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod ai_providers;
pub mod analytics;
pub mod background_tasks;
pub use background_tasks::{BackgroundTasks, DrainOutcome, OwnedTask};
pub mod analytics_events;
pub mod auth;
pub mod content;
pub use content::{ContentCatalogStats, DynContentCatalogStats};
pub mod context;
pub mod context_provider;
pub mod domain_config;
pub mod extension_error;
pub mod jwt;
pub mod log_service;
pub mod managed_resources;
pub mod ownership;
pub mod registry;
pub mod repository;
pub mod storage;
pub mod tool_executions;
pub mod validation;
pub mod validation_report;

pub use systemprompt_provider_contracts::{
    Dependencies, Job, JobContext, JobResult, JobScope, MissingDependency, ProviderError,
    ProviderResult, ServerListingFailure, ToolCallRequest, ToolCallResult, ToolContent,
    ToolContext, ToolDefinition, ToolInventory, ToolProvider, ToolProviderError,
    ToolProviderResult, submit_job,
};

pub use context::{
    AppContext, ConfigProvider, ContextPropagation, ContextPropagationError,
    ContextPropagationResult, DatabaseHandle, InjectContextHeaders,
};

pub use systemprompt_identifiers::{
    DbValue, FromDbValue, JsonRow, ToDbValue, parse_database_datetime,
};

pub use repository::{BoxedSource, ConstraintKind, RepositoryError};

pub use ownership::{DynOwnerReassignment, OwnerReassignment, ReassignedRows};
pub use tool_executions::{
    DynToolCallIntentClaims, DynToolExecutionLookup, ToolCallIntentClaims, ToolExecutionLookup,
};

pub use log_service::LogService;

pub use managed_resources::{
    DynManagedSkillResolver, ManagedSkillResolver, ManagedSkillResolverError, ResolvedManagedSkill,
    SkillResolution, WithheldReason,
};

pub use context_provider::{
    ContextMaterializer, ContextProvider, ContextProviderError, ContextStats, ContextWithStats,
    DynContextMaterializer, EnsureContextParams,
};

pub use validation::{MetadataValidation, MetadataValidationError, Validate, ValidationResult};

pub use analytics::{
    ActiveSession, AnalyticsProvider, AnalyticsProviderError, AnalyticsResult, AnalyticsSession,
    CreateSessionInput, DynSessionUsageCounters, ExtractSignals, FingerprintProvider,
    SessionAnalytics, SessionUsageCounters,
};

pub use auth::{
    AuthProviderError, AuthResult, AuthUser, FederatedIdentityClaims, RoleProvider, SenderIdentity,
    UserProvider,
};

pub use storage::{
    FileStorage, FileStorageError, FileStorageResult, StoredFileId, StoredFileMetadata,
};

pub use ai_providers::{
    AiFilePersistenceProvider, AiGeneratedFile, AiProviderError, AiProviderResult, AiRequestTrace,
    AiSessionProvider, CreateAiSessionParams, DynAiFilePersistenceProvider, DynAiSessionProvider,
    ImageGenerationInfo, ImageMetadata, ImageStorageConfig, InsertAiFileParams, TraceMessage,
    TraceRequestStatus, TraceRequestUsage, TraceSample, TraceSampleFilter, TraceSampleMode,
};

pub use registry::{
    AgentInfo, AgentRegistryProvider, McpRegistryProvider, McpServerInfo, RegistryError,
    ServiceOAuthConfig,
};

pub use extension_error::{ExtensionError, McpErrorData};

pub use domain_config::{DomainConfig, DomainConfigError, DomainConfigRegistry};

pub use validation_report::{
    StartupValidationError, StartupValidationReport, ValidationReport, ValidationWarning,
};

pub use jwt::{
    AgentJwtClaims, DynJwtValidationProvider, GenerateTokenParams, JwtProviderError, JwtResult,
    JwtValidationProvider,
};

mod startup_events;
pub use startup_events::*;

pub mod session_store;
pub use analytics::SessionProvider;
pub use analytics_events::{AnalyticsEventRecord, AnalyticsEventStore, DynAnalyticsEventStore};
pub use session_store::{DynSessionStore, SessionStore};
