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

pub mod db_value;

pub use db_value::{DbValue, FromDbValue, JsonRow, ToDbValue, parse_database_datetime};

mod actor;
mod agent;
mod ai;
mod auth;
mod client;
mod cloud;
mod connection;
mod content;
mod context;
mod email;
mod events;
mod execution;
mod funnel;
mod gateway_conversation;
mod hook;
mod jobs;
mod links;
mod locale;
mod marketplace;
mod mcp;
mod oauth;
mod path;
mod plugin;
mod policy;
mod profile;
mod provider_request;
mod roles;
mod section;
mod session;
mod task;
mod tenant;
mod trace;
mod url;
mod user;
mod webhook;

pub mod bootstrap;
pub mod error;
pub mod headers;
pub mod macros;

pub use actor::{Actor, ActorKind};
pub use agent::{AgentId, AgentName, ExternalAgentId};
pub use ai::{
    AiGatewayPolicyId, AiQuotaBucketId, AiRequestId, AiSafetyFindingId, ConfigId, MessageId,
};
pub use auth::{ApiKeyId, ApiKeySecret, CloudAuthToken, DeviceCertId, JwtToken, SessionToken};
pub use client::{ClientId, ClientType};
pub use cloud::{CheckoutSessionId, PriceId, TransactionId};
pub use connection::ConnectionId;
pub use content::{CategoryId, ContentId, FileId, SkillId, SourceId, TagId};
pub use context::ContextId;
pub use email::Email;
pub use events::EventOutboxId;
pub use execution::{ArtifactId, ExecutionStepId, LogId, TokenId};
pub use funnel::{EngagementEventId, FunnelId, FunnelProgressId};
pub use gateway_conversation::GatewayConversationId;
pub use hook::HookId;
pub use jobs::{JobName, ScheduledJobId};
pub use links::{CampaignId, LinkClickId, LinkId};
pub use locale::LocaleCode;
pub use marketplace::MarketplaceId;
pub use mcp::{AiToolCallId, McpExecutionId, McpServerId};
pub use oauth::{AccessTokenId, AuthorizationCode, ChallengeId, RefreshTokenId};
pub use path::ValidatedFilePath;
pub use plugin::PluginId;
pub use policy::PolicyVersion;
pub use profile::ProfileName;
pub use provider_request::ProviderRequestId;
pub use roles::RoleId;
pub use section::SectionId;
pub use session::{SessionId, SessionSource};
pub use task::TaskId;
pub use tenant::TenantId;
pub use trace::TraceId;
pub use url::ValidatedUrl;
pub use user::UserId;
pub use webhook::WebhookEndpointId;

define_id!(RuleId, generate);
