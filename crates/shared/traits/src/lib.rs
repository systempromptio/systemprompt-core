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

pub mod agent_run_launcher;
pub mod agent_server;
pub mod service_process;
pub use service_process::{
    DynServiceProcessSet, ServiceProcessError, ServiceProcessKind, ServiceProcessSet,
    ServiceProcessStatus,
};
pub use agent_server::{AgentServeError, AgentServer, DynAgentServer};
pub use agent_run_launcher::{
    AgentRunLaunch, AgentRunLaunchError, AgentRunLauncher, AgentRunUpdates, DynAgentRunLauncher,
};
pub mod ai_providers;
pub mod analytics;
pub mod background_tasks;
pub mod backoff;
pub use background_tasks::{BackgroundTasks, DrainOutcome, OwnedTask};
pub mod cache_epoch;
pub use cache_epoch::{CacheDomain, CacheEpochs};
pub mod provider_tokens;
pub use provider_tokens::ProviderTokenCache;
pub mod config_watch;
pub use config_watch::{CONFIG_WATCH_POLL, ConfigVersionSource, ConfigWatch, WatchSignals};
pub use stream_drain::{StreamDrain, StreamDrainWatch};
pub mod analytics_events;
pub mod auth;
pub mod authz;
pub mod content;
pub use content::{
    ContentCatalogStats, ContentPopularity, DynContentCatalogStats, DynContentPopularity,
};
pub use traffic_class_counts::{DynTrafficClassCounts, TrafficClassCounts, TrafficClassTotals};
pub mod context;
pub mod context_provider;
pub mod core_seam;
pub use core_seam::CoreSeam;
pub mod coordination;
pub mod events;
pub mod extension_error;
pub mod ip_bans;
pub mod job_scope;
pub mod jwt;
pub mod log_service;
pub mod managed_resources;
pub mod messaging;
pub use messaging::{
    DispatchOutcome, DynMessagingDispatcher, MessagingConversation, MessagingDispatchError,
    MessagingDispatcher, MessagingInbound,
};
pub mod ownership;
pub mod process_liveness;
pub mod registry;
pub mod repository;
pub mod retention_purge;
pub mod run_origin;
pub mod runner_service;
pub mod secret;
pub use process_liveness::ProcessLiveness;
pub use run_origin::RunOrigin;
pub use runner_service::RunnerService;
pub use secret::{
    ExtensionSecret, ReloadOutcome, SECRET_SEED_BYTES, SecretLookup, SecretStore, SecretStoreError,
    SecretsStatus,
};
pub mod storage;
pub mod stream_drain;
pub mod tool_executions;
pub mod traffic_class_counts;
pub mod validation;

pub use systemprompt_provider_contracts::{
    BatchPlan, BatchedRun, ChangeActor, ChangeSource, DbBudget, Dependencies, DoctorCheck,
    DoctorContext, DoctorOutcome, DoctorProbeError, DoctorProbes, DoctorStatus,
    GitCredentialSource, Job, JobContext, JobResult, JobScope, MissingDependency, ProviderError,
    ProviderResult, SecretPresence, ServerListingFailure, ToolCallRequest, ToolCallResult,
    ToolContent, ToolContext, ToolDefinition, ToolInventory, ToolProvider, ToolProviderError,
    ToolProviderResult, run_batched,
};

pub use context::{
    AppContext, ConfigProvider, ContextPropagation, ContextPropagationError,
    ContextPropagationResult, DatabaseHandle, InjectContextHeaders,
};

pub use systemprompt_identifiers::{
    DbValue, FromDbValue, JsonRow, ToDbValue, parse_database_datetime,
};

pub use repository::{BoxedSource, ConstraintKind, RepositoryError};

pub use ip_bans::{DynIpBanStore, IpBanRequest, IpBanStore, IpBanStoreError};
pub use job_scope::JobRunScope;
pub use retention_purge::{RetentionPurge, RetentionSweep};

pub use coordination::{
    AdvisoryLockGuard, AdvisoryLocks, CoordinationError, CoordinationResult, DynAdvisoryLocks,
    DynInstanceRegistry, DynLeaseRepository, HeldAdvisoryLock, InstanceRegistry, Lease,
    LeaseRepository, UnavailableCoordination,
};

pub use ownership::is_plain_identifier;
pub use tool_executions::{
    DynToolCallIntentClaims, DynToolExecutionLookup, ToolCallIntentClaims, ToolExecutionLookup,
};

pub use log_service::LogService;

pub use managed_resources::{
    DynInventoryPublisher, DynManagedSkillResolver, InventoryPublishError, InventoryPublisher,
    ManagedSkillResolver, ManagedSkillResolverError, ResolvedManagedSkill, SkillResolution,
    WithheldReason,
};

pub use context_provider::{
    ContextDirectory, ContextMaterializer, ContextProvider, ContextProviderError, ContextStats,
    ContextWithStats, DynContextDirectory, DynContextMaterializer, DynSessionContextIssuer,
    EnsureContextParams, OwnedContextSummary, SessionContextIssuer,
};

pub use validation::{MetadataValidation, MetadataValidationError, Validate, ValidationResult};

pub use analytics::{
    ActiveSession, AnalyticsProvider, AnalyticsProviderError, AnalyticsResult, AnalyticsSession,
    CreateSessionInput, DynSessionUsageCounters, ExtractSignals, FingerprintProvider,
    SessionAnalytics, SessionUsageCounters,
};

pub use auth::{
    AuthProviderError, AuthResult, AuthUser, FederatedIdentityClaims, FederatedLink,
    FederatedRefusal, FederatedResolution, LinkPolicy, RoleProvider, SenderIdentity, UserProvider,
};

pub use storage::{
    ByteRange, FileByteStream, FileStorage, FileStorageError, FileStorageResult, ServedRange,
    StoredFileId, StoredFileMetadata, StoredFileRead,
};

pub use ai_providers::{
    AiFilePersistenceProvider, AiGeneratedFile, AiProviderError, AiProviderResult, AiRequestTrace,
    AiSessionProvider, CreateAiSessionParams, DynAiFilePersistenceProvider, DynAiSessionProvider,
    ImageGenerationInfo, ImageMetadata, ImageStorageConfig, InsertAiFileParams, RequestStatus,
    TraceMessage, TraceRequestUsage, TraceSample, TraceSampleFilter, TraceSampleMode,
    UnknownRequestStatus,
};

pub use registry::{
    AgentInfo, AgentRegistryProvider, McpRegistryProvider, McpServerInfo, RegistryError,
    ServiceOAuthConfig,
};

pub use extension_error::{ExtensionError, McpErrorData};

pub use jwt::{
    AgentJwtClaims, DynJwtValidationProvider, JwtProviderError, JwtResult, JwtValidationProvider,
};

mod startup_events;
pub use startup_events::{
    ModuleInfo, Phase, ServiceInfo, ServiceState, ServiceType, StartupEvent, StartupEventExt,
    StartupEventReceiver, StartupEventSender, startup_channel,
};

pub mod session_behaviour;
pub mod session_store;
pub mod sso;
pub use analytics::SessionProvider;
pub use analytics_events::{AnalyticsEventRecord, AnalyticsEventStore, DynAnalyticsEventStore};
pub use session_behaviour::{DetectionSubject, DynSessionBehaviourScorer, SessionBehaviourScorer};
pub use session_store::{DynSessionStore, SessionStore};
