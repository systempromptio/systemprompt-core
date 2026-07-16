//! Foundation data models for systemprompt.io.
//!
//! `systemprompt-models` is the shared `shared/*` crate that every
//! other layer (`infra`, `domain`, `app`, `entry`) depends on for the
//! plain DTO and configuration shapes that flow across the system.
//! It owns the wire types of the public HTTP API, the on-disk profile
//! and services configuration, the A2A and AG-UI protocol shapes, the
//! MCP metadata helpers, and the typed error enums returned by every
//! public function in this crate.
//!
//! # Module map
//!
//! - [`a2a`] — A2A protocol agent card, message, task, and transport types.
//! - [`agui`] — AG-UI streaming event protocol.
//! - [`ai`] — LLM request/response shapes plus the [`ai::AiProvider`] trait.
//! - [`api`] — public HTTP envelopes, error model, pagination, cloud DTOs.
//! - [`artifacts`] — typed tool-result artifacts.
//! - [`auth`] — authenticated user, permission, audience, and PKCE types.
//! - [`config`] — global [`config::Config`] singleton and validation helpers.
//! - [`content`], [`content_config`] — published content metadata.
//! - [`errors`] — `thiserror`-derived public error enums.
//! - [`events`] — analytics, A2A and system event envelopes.
//! - [`execution`] — request context and execution-step bookkeeping.
//! - [`extension`] — extension framework manifest types.
//! - [`mcp`] — MCP protocol metadata helpers.
//! - [`modules`] — module manifest tree resolution.
//! - [`oauth`] — OAuth client / server config shapes.
//! - [`paths`] — well-known directory layout helpers.
//! - [`profile`] — on-disk profile and bootstrap configuration.
//! - [`repository`] — repository lifecycle traits and value objects.
//! - [`routing`] — request routing classification.
//! - [`secrets`] — secrets document model.
//! - [`services`] — services manifest (agents, plugins, hooks, MCP, …).
//! - [`users`] — public user / session summaries.
//! - [`validators`] — startup configuration validation passes.
//! - [`wire`] — canonical AI wire types and per-protocol codecs (gateway +
//!   agent clients).
//!
//! # Feature flags
//!
//! | Feature | Effect |
//! | ------- | ------ |
//! | _default_ | All public DTOs and traits, no axum integration. |
//! | `web` | Adds `axum::IntoResponse` impls for the API envelopes. |
//! | `sqlx` | Derives `sqlx::Type` on DB-persisted enums (e.g. [`ContextKind`]). |
//!
//! Public functions return `thiserror`-derived enums from [`errors`];
//! `anyhow::Error` is never used in a public signature.

pub mod macros;

pub mod a2a;
pub mod admin;
pub mod agui;
pub mod ai;
pub mod api;
pub mod artifacts;
pub mod auth;
pub mod bridge;
pub mod config;
pub mod content;
pub mod content_config;
pub mod env;
pub mod errors;
pub mod events;
pub mod execution;
pub mod extension;
pub mod gateway_hash;
pub mod mcp;
pub mod modules;
pub mod net;
pub mod oauth;
pub mod paths;
pub mod profile;
pub mod repository;
pub mod routing;
pub mod schema;
pub mod secrets;
pub mod services;
pub mod subprocess;
pub mod text;
pub mod time_format;
pub mod users;
pub mod validators;
pub mod wire;

pub use a2a::{
    AgentAuthentication, AgentCapabilities, AgentCard, AgentCardBuilder, AgentCardSignature,
    AgentExtension, AgentInterface, AgentProvider, AgentSkill, ApiKeyLocation,
    Artifact as A2aArtifact, ArtifactMetadata, ArtifactSummary, DataPart, FileContent, FilePart,
    McpServerMetadata, McpToolsParams, Message, MessageMetadata as A2aMessageMetadata,
    MessageRole as A2aMessageRole, OAuth2Flow, OAuth2Flows, Part, ProtocolBinding, SecurityScheme,
    Task, TaskMetadata, TaskState, TaskStatus, TextPart, TransportProtocol,
};
pub use admin::{
    ActivityTrend, AnalyticsData as AdminAnalyticsData, BotTrafficStats, BrowserBreakdown,
    ContentStat, DeviceBreakdown, GeographicBreakdown, LogEntry as AdminLogEntry,
    LogLevel as AdminLogLevel, RecentConversation, TrafficData as AdminTrafficData, UserInfo,
    UserMetricsWithTrends,
};
pub use agui::{
    AgUiEvent, AgUiEventBuilder, AgUiEventType, CustomPayload, GenericCustomPayload,
    JsonPatchOperation, MessageRole as AgUiMessageRole, MessagesSnapshotPayload, RunErrorPayload,
    RunFinishedPayload, RunStartedPayload, StateDeltaBuilder, StateDeltaPayload,
    StateSnapshotPayload, StepFinishedPayload, StepStartedPayload, TextMessageContentPayload,
    TextMessageEndPayload, TextMessageStartPayload, ToolCallArgsPayload, ToolCallEndPayload,
    ToolCallResultPayload, ToolCallStartPayload,
};
pub use ai::{
    AiContentPart, AiMessage, AiProvider, AiRequest, AiRequestBuilder, AiResponse, CallToolResult,
    DynAiProvider, McpTool, MessageRole, ModelConfig, ModelHint, ModelPreferences, ProviderConfig,
    ResponseFormat, SUPPORTED_AUDIO_TYPES, SUPPORTED_IMAGE_TYPES, SUPPORTED_TEXT_TYPES,
    SUPPORTED_VIDEO_TYPES, SamplingParams, SearchGroundedResponse, StreamChunk,
    StructuredOutputOptions, ToolCall, ToolExecution, ToolResultFormatter, is_supported_audio,
    is_supported_image, is_supported_media, is_supported_text, is_supported_video,
};
pub use api::{
    AcceptedResponse, ApiError, ApiErrorExt, ApiQuery, ApiResponse, CheckoutEvent, CheckoutRequest,
    CheckoutResponse, CloudApiError, CloudApiErrorDetail, CloudApiResponse, CloudCustomerInfo,
    CloudListResponse, CloudLogEntry, CloudLogsResponse, CloudPlan, CloudPlanInfo,
    CloudStatusResponse, CloudTenant, CloudTenantInfo, CloudTenantSecrets, CloudTenantStatus,
    CloudTenantStatusResponse, CloudUserInfo, CollectionResponse, ContextKind,
    CreateContextRequest, CreatedResponse, DeployResponse, DiscoveryResponse, ErrorCode,
    ErrorResponse, ExternalDbAccessResponse, Link, ModuleInfo, PaginationInfo, PaginationParams,
    ParseContextKindError, ProvisioningEvent, ProvisioningEventType, RegistryToken, ResponseLinks,
    ResponseMeta, SearchQuery, SetExternalDbAccessRequest, SetSecretsRequest, SingleResponse,
    SortOrder, SortParams, SubscriptionStatus, SuccessResponse, UpdateContextRequest, UserContext,
    UserContextWithStats, UserMeResponse, ValidationError,
};
pub use artifacts::{
    Alignment, Artifact, ArtifactSchema, ArtifactType, AudioArtifact, AxisType, ChartArtifact,
    ChartDataset, ChartType, CliArtifact, Column, ColumnType, ExecutionMetadata, ImageArtifact,
    SortOrder as ArtifactSortOrder, TableArtifact, TableHints, ToolResponse, VideoArtifact,
};
pub use auth::{
    AuthError, AuthenticatedUser, BEARER_PREFIX, BaseRole, BaseRoles, PkceMethod, ResponseType,
};
pub use config::{Config, PathNotConfiguredError};
pub use content::{ContentLink, IngestionReport};
pub use content_config::{
    ArticleDefaults, Category, ContentConfigError, ContentConfigErrors, ContentConfigRaw,
    ContentRouting, ContentSourceConfigRaw, IndexingConfig, Metadata, OrganizationData,
    ParentRoute, SitemapConfig, SourceBranding, StructuredData,
};
pub use env::{contains_placeholder, interpolate, none_if_blank, read_env_optional};
pub use errors::{RepositoryError, ServiceError};
pub use events::{
    A2AEvent, A2AEventBuilder, A2AEventType, AnalyticsEvent, AnalyticsEventBuilder, ContextEvent,
    ContextSummary, SystemEvent, SystemEventBuilder, SystemEventType,
};
pub use execution::{
    ExecutionStep, PlannedTool, RequestContext, StepContent, StepId, StepStatus, StepType,
    TrackedStep,
};
pub use extension::{
    BuildType, DiscoveredExtension, Extension, ExtensionManifest, ExtensionType, ManifestRole,
};
pub use mcp::{
    Deployment, DeploymentConfig, DynMcpDeploymentProvider, DynMcpRegistry, DynMcpToolProvider,
    ERROR as MCP_ERROR, McpAuthState, McpDeploymentProvider, McpProvider, McpRegistry,
    McpServerConfig, McpServerState, McpToolProvider, OAuthRequirement, RUNNING as MCP_RUNNING,
    RegistryConfig, STARTING as MCP_STARTING, STOPPED as MCP_STOPPED, Settings,
};
pub use modules::{ApiPaths, CliPaths, ServiceCategory};
pub use oauth::{OAuthClientConfig, OAuthServerConfig};
pub use paths::{
    AppPaths, BuildPaths, PathError, StoragePaths, SystemPaths, WebPaths, cloud_container,
    dir_names, file_names,
};
pub use profile::{
    CloudConfig, CloudValidationMode, ContentNegotiationConfig,
    DatabaseConfig as ProfileDatabaseConfig, Environment, ExtensionsConfig, LogLevel, OutputFormat,
    PathsConfig, Profile, ProfileInfo, ProfileStyle, ProfileType, RateLimitsConfig, RuntimeConfig,
    SecurityConfig, SecurityHeadersConfig, ServerConfig, SiteConfig,
};
pub use repository::{ServiceLifecycle, ServiceRecord, WhereClause};
pub use routing::{ApiCategory, AssetType, RouteClassifier, RouteType};
pub use secrets::Secrets;
pub use services::{
    AGENT_CONFIG_FILENAME, AgentCardConfig, AgentConfig, AgentMetadataConfig, AgentProviderInfo,
    AgentSummary, AiConfig, AiProviderConfig, CapabilitiesConfig, ComponentFilter, ComponentSource,
    DEFAULT_AGENT_SYSTEM_PROMPT_FILE, DEFAULT_SKILL_CONTENT_FILE, DiskAgentConfig, DiskHookConfig,
    DiskSkillConfig, Frontmatter, HOOK_CONFIG_FILENAME, HistoryConfig, HookAction, HookCategory,
    HookEvent, HookEventsConfig, HookMatcher, HookType, IncludableString, JobConfig,
    MarketplaceConfig, MarketplaceConfigFile, MarketplaceVisibility, McpConfig,
    OAuthConfig as AgentOAuthConfig, PluginAuthor, PluginComponentRef, PluginConfig,
    PluginConfigFile, PluginScript, PluginVariableDef, RuntimeStatus, SKILL_CONFIG_FILENAME,
    SamplingConfig, SchedulerConfig, ServiceType, ServicesConfig, Settings as ServicesSettings,
    SkillConfig, SkillsConfig, SystemAdmin, SystemAdminConfig, WebConfig, split_frontmatter,
    strip_frontmatter,
};
pub use systemprompt_identifiers::{AgentId, ContextId, SessionId, TaskId, TraceId, UserId};
pub use users::{SessionSummary, UserSummary};

pub use systemprompt_provider_contracts::{
    AnimationConfig, CardConfig, ColorsConfig, FontsConfig, LayoutConfig, LogoConfig, MobileConfig,
    PathsConfig as WebPathsConfig, RadiusConfig, ScriptConfig, ShadowsConfig, SpacingConfig,
    TouchTargetsConfig, TypographyConfig, WebConfigError, ZIndexConfig,
};
pub use systemprompt_traits::{
    StartupValidationError, StartupValidationReport, ValidationReport, ValidationWarning,
};
