#![allow(clippy::doc_markdown)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::ref_option)]
#![allow(clippy::range_plus_one)]

pub mod a2a;
pub mod admin;
pub mod agui;
pub mod ai;
pub mod api;
pub mod artifacts;
pub mod auth;
pub mod config;
pub mod content;
pub mod content_config;
pub mod errors;
pub mod events;
pub mod execution;
pub mod extension;
pub mod mcp;
pub mod modules;
pub mod oauth;
pub mod paths;
pub mod profile;
pub mod profile_bootstrap;
pub mod repository;
pub mod routing;
pub mod secrets;
pub mod services;
pub mod tasks;
pub mod validators;

pub use a2a::{
    AgentAuthentication, AgentCapabilities, AgentCard, AgentCardBuilder, AgentCardSignature,
    AgentExtension, AgentInterface, AgentProvider, AgentSkill, ApiKeyLocation,
    Artifact as A2aArtifact, ArtifactMetadata, DataPart, FilePart, FileWithBytes,
    McpServerMetadata, McpToolsParams, Message, MessageMetadata as A2aMessageMetadata,
    MessageRole as A2aMessageRole, OAuth2Flow, OAuth2Flows, Part, SecurityScheme, Task,
    TaskMetadata, TaskState, TaskStatus, TextPart, TransportProtocol,
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
    is_supported_audio, is_supported_image, is_supported_media, is_supported_video, AiContentPart,
    AiMessage, AiProvider, AiRequest, AiRequestBuilder, AiResponse, CallToolResult, DynAiProvider,
    McpTool, MessageRole, ModelConfig, ModelHint, ModelPreferences, ProviderConfig, ResponseFormat,
    SamplingParams, SearchGroundedResponse, StructuredOutputOptions, ToolCall, ToolExecution,
    ToolResultFormatter, SUPPORTED_AUDIO_TYPES, SUPPORTED_IMAGE_TYPES, SUPPORTED_VIDEO_TYPES,
};
pub use api::{
    AcceptedResponse, ApiError, ApiQuery, ApiResponse, CheckoutEvent, CheckoutRequest,
    CheckoutResponse, CloudApiError, CloudApiErrorDetail, CloudApiResponse, CloudCustomerInfo,
    CloudListResponse, CloudLogEntry, CloudLogsResponse, CloudPlan, CloudPlanInfo,
    CloudStatusResponse, CloudTenant, CloudTenantInfo, CloudTenantSecrets, CloudTenantStatus,
    CloudTenantStatusResponse, CloudUserInfo, CollectionResponse, CreateContextRequest,
    CreatedResponse, DeployResponse, DiscoveryResponse, ErrorCode, ErrorResponse,
    ExternalDbAccessResponse, Link, ModuleInfo, PaginationInfo, PaginationParams,
    ProvisioningEvent, ProvisioningEventType, RegistryToken, ResponseLinks, ResponseMeta,
    SearchQuery, SetExternalDbAccessRequest, SetSecretsRequest, SingleResponse, SortOrder,
    SortParams, SubscriptionStatus, SuccessResponse, UpdateContextRequest, UserContext,
    UserContextWithStats, UserMeResponse, ValidationError,
};
pub use artifacts::{
    Alignment, Artifact, ArtifactSchema, ArtifactType, AxisType, ChartArtifact, ChartDataset,
    ChartType, Column, ColumnType, ExecutionMetadata, SortOrder as ArtifactSortOrder,
    TableArtifact, TableHints,
};
pub use auth::{
    AuthError, AuthenticatedUser, BaseRole, BaseRoles, GrantType, PkceMethod, ResponseType,
    BEARER_PREFIX,
};
pub use config::{Config, PathNotConfiguredError};
pub use content::{ContentLink, IngestionReport};
pub use content_config::{
    ArticleDefaults, Category, ContentConfigError, ContentConfigErrors, ContentConfigRaw,
    ContentRouting, ContentSourceConfigRaw, IndexingConfig, Metadata, OrganizationData,
    ParentRoute, SitemapConfig, SourceBranding, StructuredData,
};
pub use errors::{CoreError, RepositoryError, ServiceError};
pub use events::{
    A2AEvent, A2AEventBuilder, A2AEventType, ContextEvent, ContextSummary, SystemEvent,
    SystemEventBuilder, SystemEventType, ToSse,
};
pub use execution::{
    ExecutionStep, PlannedTool, RequestContext, StepContent, StepId, StepStatus, StepType,
    TrackedStep,
};
pub use extension::{BuildType, DiscoveredExtension, Extension, ExtensionManifest, ExtensionType};
pub use mcp::{
    Deployment, DeploymentConfig, DynMcpDeploymentProvider, DynMcpRegistry, DynMcpToolProvider,
    McpAuthState, McpDeploymentProvider, McpProvider, McpRegistry, McpServerConfig, McpServerState,
    McpToolProvider, OAuthRequirement, RegistryConfig, Settings, ERROR as MCP_ERROR,
    RUNNING as MCP_RUNNING, STARTING as MCP_STARTING, STOPPED as MCP_STOPPED,
};
pub use modules::{
    ApiConfig, ApiPaths, Module, ModuleDefinition, ModulePermission, ModuleSchema, ModuleSeed,
    ModuleType, Modules, ServiceCategory,
};
pub use oauth::{OAuthClientConfig, OAuthServerConfig};
pub use paths::{AppPaths, BuildPaths, PathError, SystemPaths, WebPaths};
pub use profile::{
    CloudConfig, CloudValidationMode, DatabaseConfig as ProfileDatabaseConfig, Environment,
    LogLevel, OutputFormat, PathsConfig, Profile, ProfileStyle, ProfileType, RateLimitsConfig,
    RuntimeConfig, SecurityConfig, ServerConfig, SiteConfig,
};
pub use profile_bootstrap::{ProfileBootstrap, ProfileBootstrapError};
pub use repository::{ServiceLifecycle, ServiceRecord, WhereClause};
pub use routing::{ApiCategory, AssetType, RouteClassifier, RouteType};
pub use secrets::{Secrets, SecretsBootstrap, SecretsBootstrapError};
pub use services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, AgentProviderInfo, AiConfig,
    AiProviderConfig, CapabilitiesConfig, HistoryConfig, IncludableString, JobConfig, McpConfig,
    OAuthConfig as AgentOAuthConfig, PartialServicesConfig, RuntimeStatus, SamplingConfig,
    SchedulerConfig, ServiceType, ServicesConfig, Settings as ServicesSettings, SkillConfig,
    SkillsConfig, ToolModelConfig, ToolModelSettings, WebConfig,
};
pub use systemprompt_identifiers::{AgentId, ContextId, SessionId, TaskId, TraceId, UserId};
pub use tasks::{TaskMessage, TaskRecord};

pub use systemprompt_traits::{
    StartupValidationError, StartupValidationReport, ValidationReport, ValidationWarning,
};
