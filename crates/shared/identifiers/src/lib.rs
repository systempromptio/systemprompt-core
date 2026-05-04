//! Typed newtype identifiers for systemprompt.io.
//!
//! Every entity in the platform is referenced through a wrapper newtype
//! rather than a raw `String`.
//!
//! This crate provides both the macros that generate those wrappers
//! ([`define_id!`], [`define_token!`]) and the canonical concrete types
//! (`UserId`, `AgentId`, `TaskId`, `TraceId`, `ContextId`, `SessionId`,
//! `McpServerId`, ...).
//!
//! Boundary types for talking to the database — [`DbValue`], [`ToDbValue`],
//! [`FromDbValue`], [`JsonRow`] — also live here so that identifier modules
//! can interoperate without depending on the database crate.
//!
//! # Construction
//!
//! ```ignore
//! use systemprompt_identifiers::{TaskId, UserId};
//!
//! // Known string value (literal, parsed input, DB row).
//! let user = UserId::new("user_abc");
//!
//! // Mint a fresh UUID-backed identifier.
//! let task = TaskId::generate();
//! ```
//!
//! Validated identifiers (`McpServerId`, `Email`, `ProfileName`,
//! `ValidatedUrl`, `ValidatedFilePath`, `AgentName`) additionally expose a
//! fallible `try_new` constructor returning [`error::IdValidationError`].
//!
//! # Feature flags
//!
//! | Feature | Effect |
//! |---------|--------|
//! | (default) | Pure-Rust types only. |
//! | `sqlx` | Derives `sqlx::Type` on every identifier, allowing direct binding in `query_as!` macros. |

/// Database-value enum and conversion traits.
///
/// Used by repository code to shuttle scalar values between Rust and the
/// underlying SQL driver.
pub mod db_value;

pub use db_value::{DbValue, FromDbValue, JsonRow, ToDbValue, parse_database_datetime};

mod agent;
mod ai;
mod auth;
mod client;
mod content;
mod context;
mod email;
mod execution;
mod funnel;
mod hook;
mod jobs;
mod links;
mod mcp;
mod oauth;
mod path;
mod plugin;
mod policy;
mod profile;
mod roles;
mod session;
mod task;
mod tenant;
mod trace;
mod url;
mod user;

/// Error types produced by identifier validation and database value
/// conversion.
pub mod error;
/// HTTP header name constants used throughout the platform.
pub mod headers;
/// Declarative macros that generate typed identifier and token newtypes.
pub mod macros;

/// Agent identifiers — `AgentId` (UUID-backed) and `AgentName` (validated,
/// non-empty, reserves `"unknown"`).
pub use agent::{AgentId, AgentName};
/// AI subsystem identifiers covering requests, messages, configs, safety
/// findings, quota buckets, and gateway policies.
pub use ai::{
    AiGatewayPolicyId, AiQuotaBucketId, AiRequestId, AiSafetyFindingId, ConfigId, MessageId,
};
/// Authentication identifiers and opaque tokens (API keys, JWT, session and
/// cloud bearer tokens, device certificates).
pub use auth::{ApiKeyId, ApiKeySecret, CloudAuthToken, DeviceCertId, JwtToken, SessionToken};
/// OAuth client identifier with classifier helpers (`is_dcr`, `is_cimd`,
/// `is_system`).
pub use client::{ClientId, ClientType};
/// Content management identifiers (categories, content rows, files, skills,
/// sources, tags).
pub use content::{CategoryId, ContentId, FileId, SkillId, SourceId, TagId};
/// Execution-context identifier (one per logical conversation/task tree).
pub use context::ContextId;
/// Validated email address with structural checks on local-part and domain.
pub use email::Email;
/// Execution-trace identifiers (steps, log entries, AI tokens, A2A
/// artifacts).
pub use execution::{ArtifactId, ExecutionStepId, LogId, TokenId};
/// Marketing-funnel identifiers (engagement events, funnels, progress rows).
pub use funnel::{EngagementEventId, FunnelId, FunnelProgressId};
/// Hook identifier for pluggable extension callbacks.
pub use hook::HookId;
/// Scheduler-job identifiers (`JobName`, `ScheduledJobId`).
pub use jobs::{JobName, ScheduledJobId};
/// Link-tracking identifiers (campaigns, links, click events).
pub use links::{CampaignId, LinkClickId, LinkId};
/// MCP-protocol identifiers (server, execution, tool-call).
pub use mcp::{AiToolCallId, McpExecutionId, McpServerId};
/// OAuth flow identifiers (access/refresh tokens, authorization codes,
/// PKCE challenges).
pub use oauth::{AccessTokenId, AuthorizationCode, ChallengeId, RefreshTokenId};
/// Validated filesystem path with traversal-attack rejection.
pub use path::ValidatedFilePath;
/// Plugin identifier.
pub use plugin::PluginId;
/// Policy version identifier with an `unversioned()` constant.
pub use policy::PolicyVersion;
/// Profile name (alphanumeric/`-`/`_`) used for configuration profile
/// selection.
pub use profile::ProfileName;
/// Role identifier.
pub use roles::RoleId;
/// Session identifier (UUID-backed, `sess_` prefixed) and its
/// originating-source enum.
pub use session::{SessionId, SessionSource};
/// Task identifier.
pub use task::TaskId;
/// Tenant identifier.
pub use tenant::TenantId;
/// Distributed-tracing identifier with a `system()` constant.
pub use trace::TraceId;
/// Validated URL with scheme/host structural checks.
pub use url::ValidatedUrl;
/// User identifier with `anonymous()` and `system()` constants.
pub use user::UserId;
