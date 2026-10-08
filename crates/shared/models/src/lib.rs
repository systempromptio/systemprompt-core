//! Foundation data models for systemprompt.io.
//!
//! `systemprompt-models` is the shared `shared/*` crate that every
//! other layer (`infra`, `domain`, `app`, `entry`) depends on for the
//! plain DTO and configuration shapes that flow across the system.
//! It owns the wire types of the public HTTP API, the bridge manifest, the
//! A2A and AG-UI protocol shapes, the MCP metadata helpers, and the typed
//! error enums returned by every public function in this crate. The services
//! manifest and the on-disk profile live in `systemprompt-manifest`; the
//! provider wire codecs in `systemprompt-wire`.
//!
//! # Module map
//!
//! - [`a2a`] — A2A protocol agent card, message, task, and transport types.
//! - [`agui`] — AG-UI streaming event protocol.
//! - [`ai`] — LLM request/response shapes plus the [`ai::AiProvider`] trait.
//! - [`api`] — public HTTP envelopes, error model, pagination, cloud DTOs.
//! - [`artifacts`] — typed tool-result artifacts.
//! - [`attribution`] — scope attribution of an AI request (one value per
//!   registered subject dimension, plus the API key).
//! - [`auth`] — authenticated user, permission, audience, and PKCE types.
//! - [`bridge`] — bridge wire formats (signed manifest, plugin bundles,
//!   telemetry).
//! - [`content`], [`content_config`] — published content metadata.
//! - [`errors`] — `thiserror`-derived public error enums.
//! - [`events`] — analytics, A2A and system event envelopes.
//! - [`execution`] — request context and execution-step bookkeeping.
//! - [`extension`] — extension framework manifest types.
//! - [`hooks`] — hook lifecycle events and categories.
//! - [`macros`] — builder-setter macros used by the crate's builder types.
//! - [`managed`] — verified managed-resource revision bundles.
//! - [`mcp`] — MCP protocol metadata helpers.
//! - [`mime`] — canonical file-extension ↔ MIME mapping.
//! - [`modules`] — module manifest tree resolution.
//! - [`net`] — timeout constants and the outbound-URL (SSRF) validator.
//! - [`oauth`] — OAuth client / server config shapes.
//! - [`origin`] — client attribution of an AI request (client kind, attestation
//!   tier, evidence).
//! - [`plugin`] — plugin component references carried in the bridge manifest.
//! - [`providers`] — the client-facing API surface of an upstream provider.
//! - [`routing`] — request routing classification.
//! - [`scope`] — per-request scoping identity for scoped DB transactions.
//! - [`subprocess`] — identity contract for supervised child processes.
//! - [`text`], [`time_format`] — display formatting helpers.
//! - [`users`] — public user / session summaries.
//!
//! No module here spawns processes or opens sockets: process spawning lives
//! in `systemprompt-loader` and outbound HTTP in `systemprompt-client`.
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
pub mod attribution;
pub mod auth;
pub mod bridge;
pub mod content;
pub mod content_config;
pub mod errors;
pub mod events;
pub mod execution;
pub mod extension;
pub mod feedback;
pub mod hooks;
pub mod managed;
pub mod mcp;
pub mod mime;
pub mod modules;
pub mod net;
pub mod oauth;
pub mod origin;
pub mod plugin;
pub mod providers;
pub mod routing;
pub mod scope;
pub mod subprocess;
pub mod text;
pub mod time_format;
pub mod users;

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
pub use auth::{AuthRequestError, BaseRoles};
pub use content::IngestionReport;
pub use content_config::{
    Category, ContentConfigError, ContentConfigErrors, ContentConfigRaw, ContentRouting,
    ContentSourceConfigRaw, IndexingConfig, Metadata, ParentRoute, SitemapConfig, SourceBranding,
};
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
pub use routing::{RouteClassifier, RouteType};
pub use scope::RequestScope;
pub use systemprompt_identifiers::{ContextId, SessionId, TaskId, UserId};

pub use systemprompt_provider_contracts::WebConfigError;
