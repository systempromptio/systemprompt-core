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
//! - [`bridge`] — bridge wire formats (signed manifest, plugin bundles,
//!   telemetry).
//! - [`config`] — global [`config::Config`] singleton and validation helpers.
//! - [`content`], [`content_config`] — published content metadata.
//! - [`env`](mod@env) — `${VAR}` interpolation over environment variables.
//! - [`errors`] — `thiserror`-derived public error enums.
//! - [`events`] — analytics, A2A and system event envelopes.
//! - [`execution`] — request context and execution-step bookkeeping.
//! - [`extension`] — extension framework manifest types.
//! - [`gateway_hash`] — deterministic gateway conversation-id derivation.
//! - [`macros`] — builder-setter macros used by the crate's builder types.
//! - [`managed`] — verified managed-resource revision bundles.
//! - [`mcp`] — MCP protocol metadata helpers.
//! - [`mime`] — canonical file-extension ↔ MIME mapping.
//! - [`modules`] — module manifest tree resolution.
//! - [`net`] — timeout constants and the outbound-URL (SSRF) validator.
//! - [`oauth`] — OAuth client / server config shapes.
//! - [`paths`] — path-resolution contract and well-known directory constants.
//! - [`profile`] — on-disk profile and bootstrap configuration.
//! - [`routing`] — request routing classification.
//! - [`schema`] — JSON-Schema capability matrices and sanitisation.
//! - [`scope`] — per-request scoping identity for scoped DB transactions.
//! - [`secrets`] — secrets document model.
//! - [`services`] — services manifest (agents, plugins, hooks, MCP, …).
//! - [`subprocess`] — identity contract for supervised child processes.
//! - [`text`], [`time_format`] — display formatting helpers.
//! - [`users`] — public user / session summaries.
//! - [`validators`] — startup configuration validation passes.
//! - [`wire`] — canonical AI wire types and per-protocol codecs (gateway +
//!   agent clients).
//!
//! No module here spawns processes or opens sockets: process spawning lives
//! in `systemprompt-loader`, outbound HTTP in `systemprompt-client`, and path
//! resolution against the filesystem in `systemprompt-config`.
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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod macros;

pub mod a2a;
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
pub mod feedback;
pub mod gateway_hash;
pub mod managed;
pub mod mcp;
pub mod mime;
pub mod modules;
pub mod net;
pub mod oauth;
pub mod paths;
pub mod profile;
pub mod routing;
pub mod schema;
pub mod scope;
pub mod secrets;
pub mod services;
pub mod subprocess;
pub mod text;
pub mod time_format;
pub mod users;
pub mod validators;
pub mod wire;

pub use a2a::{
    AgentCapabilities, AgentCard, AgentExtension, AgentProvider, Artifact as A2aArtifact,
    ArtifactMetadata, DataPart, FileContent, FilePart, Part, Task, TaskMetadata, TaskState,
    TaskStatus, TextPart,
};
pub use agui::{
    AgUiEvent, AgUiEventBuilder, AgUiEventType, CustomPayload, GenericCustomPayload,
    JsonPatchOperation, MessageRole as AgUiMessageRole, MessagesSnapshotPayload, RunErrorPayload,
    RunFinishedPayload, RunStartedPayload, StateDeltaBuilder, StateDeltaPayload,
    StateSnapshotPayload,
};
pub use ai::{
    AiContentPart, AiMessage, AiProvider, AiRequest, CallToolResult, McpTool, MessageRole,
    ProviderConfig, SamplingParams, StreamChunk, ToolCall, ToolResultFormatter, is_supported_audio,
    is_supported_image, is_supported_text, is_supported_video,
};
pub use api::{
    AcceptedResponse, ApiError, ApiErrorExt, ApiResponse, CollectionResponse, ContextKind,
    CreateContextRequest, CreatedResponse, ErrorCode, PaginationInfo, PaginationParams,
    SearchQuery, SingleResponse, SortOrder, SortParams, SuccessResponse, UpdateContextRequest,
    UserContext, UserContextWithStats, ValidationError,
};
pub use artifacts::{
    Alignment, ArtifactType, AxisType, ChartType, CliArtifact, ColumnType, TableArtifact,
};
pub use auth::{AuthError, BaseRoles};
pub use config::{Config, PathNotConfiguredError};
pub use content::IngestionReport;
pub use content_config::{
    Category, ContentConfigError, ContentConfigErrors, ContentConfigRaw, ContentRouting,
    ContentSourceConfigRaw, IndexingConfig, Metadata, ParentRoute, SitemapConfig, SourceBranding,
};
pub use env::{contains_placeholder, interpolate, none_if_blank, read_env_optional};
pub use events::{
    A2AEvent, A2AEventBuilder, A2AEventType, AnalyticsEvent, AnalyticsEventBuilder, ContextEvent,
    SystemEvent, SystemEventBuilder, SystemEventType,
};
pub use execution::{
    ExecutionStep, PlannedTool, RequestContext, StepContent, StepId, StepStatus, StepType,
    TrackedStep,
};
pub use extension::{BuildType, DiscoveredExtension, ExtensionManifest};
pub use mcp::{Deployment, McpServerConfig};
pub use modules::{ApiPaths, CliPaths};
pub use paths::PathResolution;
pub use profile::{
    CloudConfig, CloudValidationMode, ContentNegotiationConfig,
    DatabaseConfig as ProfileDatabaseConfig, Environment, ExtensionsConfig, LogLevel, OutputFormat,
    PathsConfig, Profile, ProfileStyle, ProfileType, RateLimitsConfig, RuntimeConfig,
    SecurityConfig, SecurityHeadersConfig, ServerConfig, SiteConfig,
};
pub use routing::{RouteClassifier, RouteType};
pub use scope::RequestScope;
pub use secrets::Secrets;
pub use services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, AiConfig, CapabilitiesConfig,
    ComponentFilter, ComponentSource, DiskHookConfig, DiskSkillConfig, HOOK_CONFIG_FILENAME,
    MarketplaceConfig, OAuthConfig as AgentOAuthConfig, PluginComponentRef, PluginConfig,
    PluginConfigFile, PluginVariableDef, RuntimeStatus, SKILL_CONFIG_FILENAME, SchedulerConfig,
    ServiceModule, ServiceStatus, ServiceType, ServicesConfig, SkillsConfig, SystemAdmin,
    WebConfig, split_frontmatter, strip_frontmatter,
};
pub use systemprompt_identifiers::{ContextId, SessionId, TaskId, UserId};

pub use systemprompt_provider_contracts::WebConfigError;
