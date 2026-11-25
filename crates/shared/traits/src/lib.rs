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
//! [`FileStorageError`], [`ContextPropagationError`]). The crate also defines
//! the cross-cutting [`ExtensionError`] trait which downstream errors implement
//! so the API and MCP transports can render them uniformly.
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
pub mod ai_reports;
pub mod service_process;
pub use agent_run_launcher::{
    AgentRunLaunch, AgentRunLaunchError, AgentRunLauncher, AgentRunUpdates, DynAgentRunLauncher,
};
pub use agent_server::{AgentServeError, AgentServer, DynAgentServer};
pub use service_process::{
    DynServiceProcessSet, RestartMode, ServiceDetail, ServiceHealth, ServiceProcessError,
    ServiceProcessKind, ServiceProcessSet, ServiceProcessStatus, ServiceRun, StatusDepth, StopMode,
};
pub mod ai_providers;
pub mod analytics;
pub mod background_tasks;
pub mod backoff;
pub mod bridge_host_filters;
pub use background_tasks::{BackgroundTasks, DrainOutcome, OwnedTask};
pub mod cache_epoch;
pub use cache_epoch::{CacheDomain, CacheEpochView, CacheEpochs};
pub mod provider_tokens;
pub use provider_tokens::ProviderTokenCache;
pub mod config_watch;
pub mod held;
pub use config_watch::{CONFIG_WATCH_POLL, ConfigVersionSource, ConfigWatch, WatchSignals};
pub use held::Held;
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
pub mod drive_tokens;
pub mod escalation;
pub mod events;
pub mod extension_error;
pub mod gateway_session;
pub mod ip_bans;
pub mod job_scope;
pub mod log_service;
pub mod managed_resources;
pub mod messaging;
pub use messaging::{
    DispatchOutcome, DynMessagingDispatcher, MessagingConversation, MessagingDispatchError,
    MessagingDispatcher, MessagingInbound,
};
pub mod ownership;
pub mod process_liveness;
pub mod rate_limit_buckets;
pub use rate_limit_buckets::{DynRateLimitBuckets, RateLimitBuckets};
pub mod admin_bootstrap;
pub use admin_bootstrap::{
    AdminAccount, AdminBootstrap, AdminBootstrapError, AdminGrant, AdminPromotion,
    DynAdminBootstrap,
};
pub mod files_config;
pub mod plugin_token;
pub use plugin_token::{
    DynPluginTokenMinter, MintedPluginToken, PLUGIN_TOKEN_MAX_DAYS, PluginTokenHolder,
    PluginTokenMintError, PluginTokenMinter, PluginTokenRequest,
};
pub mod registry;
pub mod repository;
pub mod request_analytics;
pub mod retention_purge;
pub mod run_origin;
pub mod runner_service;
pub mod secret;
pub mod secret_pattern;
pub use files_config::FilesConfigView;
pub mod service_accounts;
pub use process_liveness::ProcessLiveness;
pub use run_origin::RunOrigin;
pub use runner_service::RunnerService;
pub use secret::{
    AtRestPepper, DeclaredSecrets, ExtensionSecret, PINNED_SECRET_KEYS, ReloadOutcome,
    SECRET_SEED_BYTES, SecretLookup, SecretReferences, SecretStore, SecretStoreError,
    SecretsStatus,
};
pub use secret_pattern::{
    CORE_SECRET_NAMES, LICENCE_SIGNING_SECRET, MCP_CREDENTIAL_BROKER_SECRET, SecretPattern,
    SecretPatternError, is_core_secret_name,
};
pub use service_accounts::{
    DynServiceAccounts, ServiceAccount, ServiceAccountError, ServiceAccountRequest, ServiceAccounts,
};
pub mod storage;
pub mod stream_drain;
pub mod tool_executions;
pub mod traffic_class_counts;
pub mod user_retention;
pub mod validation;

pub use systemprompt_provider_contracts::{
    BatchPlan, BatchedRun, ChangeActor, ChangeSource, DbBudget, Dependencies, DoctorCheck,
    DoctorContext, DoctorOutcome, DoctorProbeError, DoctorProbes, DoctorStatus,
    DynGitCredentialDirectory, GitCredentialDirectory, GitCredentialReadError, GitCredentialSource,
    Job, JobContext, JobResult, JobScope, MissedRunPolicy, MissingDependency, ProviderError,
    ProviderResult, SecretPresence, ServerListingFailure, StateRequirement, ToolCallRequest,
    ToolCallResult, ToolContent, ToolContext, ToolDefinition, ToolInventory, ToolProvider,
    ToolProviderError, ToolProviderResult, run_batched,
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
    DynInstanceRegistry, DynLeaseRepository, FencingToken, HeldAdvisoryLock, InstanceRegistry,
    Lease, LeaseRepository, UnavailableCoordination,
};

pub use ai_reports::{
    CatalogMigrationPort, DynCatalogMigrationPort, DynSafetyFindingsReader, LegacyCatalogEntry,
    SafetyFindingRollup, SafetyFindingsReader,
};
pub use bridge_host_filters::{BridgeHostFilterMigration, DynBridgeHostFilterMigration};
pub use ownership::{MERGE_EXCLUDED_SECURITY_TABLES, is_plain_identifier};
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
    CallerContexts, CallerSessionContexts, ContextDirectory, ContextMaterializer, ContextProvider,
    ContextProviderError, ContextStats, ContextWithStats, DynContextDirectory,
    DynContextMaterializer, DynSessionContextIssuer, EnsureContextParams, OwnedContextSummary,
    SessionContextIssuer,
};

pub use validation::{MetadataValidation, MetadataValidationError, Validate, ValidationResult};

pub use analytics::{
    ActiveSession, AnalyticsProvider, AnalyticsProviderError, AnalyticsResult, AnalyticsSession,
    CreateSessionInput, DynSessionUsageCounters, ExtractSignals, FingerprintProvider,
    SessionAnalytics, SessionUsageCounters,
};

pub use auth::{
    AuthProviderError, AuthResult, AuthUser, DynUserProvider, FederatedIdentityClaims,
    FederatedLink, FederatedRefusal, FederatedResolution, LinkPolicy, RoleProvider, SenderIdentity,
    UserProvider,
};

pub use storage::{
    ByteRange, FileByteStream, FileStorage, FileStorageError, FileStorageResult, ServedRange,
    StorageValidation, StoredFileId, StoredFileMetadata, StoredFileRead,
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

mod startup_events;
pub use startup_events::{
    ModuleInfo, Phase, ServiceInfo, ServiceState, ServiceType, StartupEvent, StartupEventExt,
    StartupEventReceiver, StartupEventSender, startup_channel,
};

pub mod session_behaviour;
pub mod session_issuer;
pub mod session_store;
pub mod sso;
pub use analytics::{DynSignInSessions, SessionProvider};
pub use analytics_events::{AnalyticsEventRecord, AnalyticsEventStore, DynAnalyticsEventStore};
pub use drive_tokens::{
    DriveSyncAttendance, DriveSyncGrant, DriveSyncPrincipal, DriveTokenError, DriveTokens,
    DynDriveTokens,
};
pub use escalation::EscalationDelivery;
pub use gateway_session::{
    DynGatewaySessionMint, GatewaySessionMint, GatewaySessionMintError, MintedGatewaySession,
};
pub use request_analytics::{
    DynRequestAnalytics, RequestAnalytics, RequestAnalyticsError, RequestAnalyticsEvent,
};
pub use session_behaviour::{DetectionSubject, DynSessionBehaviourScorer, SessionBehaviourScorer};
pub use session_issuer::{
    BrowserSession, DynSessionIssuer, IssuedSession, SessionIssueError, SessionIssuer,
};
pub use session_store::{DynSessionStore, DynSessionTelemetry, SessionStore, SessionTelemetry};
pub use user_retention::{ArchivedUserPurge, DynUserRetention, UserRetention};
