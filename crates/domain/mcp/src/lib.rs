//! Native Model Context Protocol (MCP) implementation for systemprompt.io.
//!
//! This crate holds the registry/orchestrator that supervises per-tenant MCP
//! child processes, the MCP client, the tool execution and artifact pipeline,
//! the repositories and the UI-renderer abstractions. The toolset host and its
//! hop/RBAC/session middleware are `systemprompt-mcp-host`'s; this crate
//! provides them the session store (`DynMcpSessionStore`, sealed) and
//! contributes to the access-recorder slot of the extension contract.
//!
//! # Layered components
//!
//! - [`extension::McpExtension`] — the MCP member's contributions.
//! - [`contract`] — the [`UiSlot`](contract::UiSlot) contribution point.
//! - [`services::McpOrchestrator`] — top-level service supervisor.
//! - [`services::registry::RegistryService`] — registry of MCP servers
//!   configured via `services.yaml`.
//! - [`services::tool_provider::McpToolProvider`] — tool-discovery + execution
//!   facade.
//! - [`repository`] — Postgres persistence for sessions, artifacts, tool usage.
//!
//! # Feature matrix
//!
//! This crate has no Cargo features today; it is built as a single unit. The
//! facade crate `systemprompt` gates this crate behind the `mcp` / `full`
//! features.
//!
//! # Errors
//!
//! All public APIs return [`McpDomainResult`] — a typed `Result` aliased over
//! [`McpDomainError`]. External error types (`sqlx`, `serde_json`, `io`,
//! join errors) are composed via `#[from]` on the error enum. Third-party
//! errors without a typed adapter keep their cause in a `#[source]` variant
//! such as `McpDomainError::Operation { context, source }`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod client_profile;
pub mod contract;
pub(crate) mod error;
pub(crate) mod extension;
pub mod jobs;
pub mod models;
pub(crate) mod progress;
pub mod repository;
pub(crate) mod resources;
pub(crate) mod response;
pub mod services;
pub(crate) mod tool;

pub use extension::McpExtension;

pub use error::{McpDomainError, McpDomainResult, ServiceStartFailure, ServiceStartFailures};
pub use rmcp::ErrorData as McpError;
pub use rmcp::model::CallToolResult;

pub type McpResult<T> = Result<T, McpError>;

pub use client_profile::client_profile_from_stored;
pub use progress::{ProgressCallback, create_progress_callback};
pub use repository::{
    ArtifactCorrelation, ArtifactFinding, ArtifactFindingRecord, ArtifactFindingRepository,
    ArtifactIngestRepositories, ArtifactPayloadRecord, ArtifactPayloadRepository, ArtifactShape,
    CreateMcpArtifact, IngestMark, IngestMarkKey, McpArtifactRecord, McpArtifactRepository,
    PHASE_TOOL_RESULT, ToolResultIngestMarkRepository,
};
pub use resources::{
    ArtifactViewerConfig, build_artifact_viewer_resource, build_resource_template_list_result,
    read_artifact_resource, read_artifact_viewer_resource,
};
pub use response::{
    McpResponseBuilder, ResponseServices, ToolIdentity, UI_RESOURCE_URI_META_KEY,
    terse_execution_meta,
};
pub use services::artifact_ingest::{
    ArtifactIngest, ArtifactScanner, FINGERPRINT_WINDOW_SECONDS, IngestOutcome, IngestRequest,
    MAX_PAYLOAD_BYTES, ScanOutcome, from_canonical_tool_result, from_hook_failure,
    from_hook_response, from_wire_value,
};
pub use services::intent_claim::IntentClaimService;
pub use services::ui_renderer::templates::html::artifact_shell_template;
pub use services::ui_renderer::{artifact_resource_uri, parse_artifact_resource_uri};
pub use tool::{INTENT_CLAIM_WINDOW_SECONDS, McpToolExecutions, McpToolExecutor};

pub use systemprompt_models::mcp::{
    Deployment, DeploymentConfig, McpAuthState, McpServerConfig, OAuthRequirement, Settings,
};

pub use services::monitoring::health::HealthStatus;
pub use services::monitoring::status::McpServiceStatus;
pub use services::registry::{McpDeploymentProviderImpl, RegistryService};
pub use services::tool_provider::McpToolProvider;
pub use services::{
    EventBus as McpEventBus, McpEvent, McpOrchestrator, McpRestartOutcome, McpSpawn,
};

pub use systemprompt_manifest::services::McpDeploymentProvider;
pub use systemprompt_models::mcp::{McpRegistry, McpServerState};

pub fn mcp_protocol_version() -> String {
    mcp_protocol_version_str().to_owned()
}

pub const fn mcp_supported_protocol_versions() -> &'static [ProtocolVersion] {
    ProtocolVersion::KNOWN_VERSIONS
}

pub const fn mcp_protocol_version_str() -> &'static str {
    "2026-07-28"
}

pub mod registry {
    pub use crate::services::registry::RegistryService;
}

pub(crate) mod state;

pub use rmcp::model::ProtocolVersion;
pub use state::McpState;
