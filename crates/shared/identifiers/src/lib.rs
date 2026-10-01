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
//! // A value already known to be valid (a decoded DB row).
//! let task = TaskId::new("task_abc");
//!
//! // A value from outside (header, path segment, JWT claim): validate it.
//! let user = UserId::try_new(raw_sub)?;
//!
//! // Mint a fresh UUID-backed identifier.
//! let fresh = UserId::generate();
//! ```
//!
//! Validated identifiers (`ContextId`, `Email`, `ProfileName`, `ValidatedUrl`,
//! `ValidatedFilePath`, `RoleId`, ...) expose **only** the fallible `try_new`
//! constructor returning [`error::IdValidationError`]; there is no
//! infallible `new` that could panic on runtime input. Values
//! minted by the platform itself (`ContextId::generate`, `AgentName::system`)
//! come from dedicated constructors.
//!
//! Checked identifiers (`UserId`, `ServiceName`, `AgentName`, `McpServerId`,
//! `McpToolName`) have both: `try_new` validates input from outside, `new`
//! accepts a value already known valid (a decoded row, a configuration key
//! validated at load). They implement no `From<String>`, and their
//! `Deserialize`/`FromStr` validate.
//!
//! "Absent" is `Option<Id>`, never a sentinel value such as `"unset"`,
//! `"unknown"` or the empty string.
//!
//! # Feature flags
//!
//! | Feature | Effect |
//! |---------|--------|
//! | (default) | Pure-Rust types only. |
//! | `sqlx` | Derives `sqlx::Type` on every identifier, allowing direct binding in `query_as!` macros. |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod db_value;

pub use db_value::{DbValue, FromDbValue, JsonRow, ToDbValue, parse_database_datetime};

mod actor;
mod agent;
mod ai;
mod auth;
mod client;
mod client_session;
mod cloud;
mod connection;
mod content;
mod context;
mod email;
mod engagement;
mod events;
mod execution;
mod gateway_boot;
mod gateway_conversation;
mod hook;
mod instance;
mod jobs;
mod links;
mod locale;
mod managed;
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
mod service;
mod session;
mod slack;
mod task;
mod teams;
mod tenant;
mod trace;
mod url;
mod user;
mod webhook;

pub mod error;
pub mod headers;
pub mod macros;

pub use actor::{Actor, ActorKind, ActorKindTag};
pub use agent::{AgentId, AgentName, ExternalAgentId};
pub use ai::{
    AiGatewayPolicyId, AiQuotaBucketId, AiRequestId, AiSafetyFindingId, ConfigId, MessageId,
};
pub use auth::{
    ApiKeyId, ApiKeySecret, CloudAuthToken, DeviceCertId, DeviceId, JwtToken, SessionToken,
};
pub use client::{ClientId, ClientType};
pub use client_session::ClientSessionId;
pub use cloud::{CloudUserId, PriceId};
pub use connection::ConnectionId;
pub use content::{CategoryId, ContentId, FileId, SkillId, SourceId, TagId};
pub use context::ContextId;
pub use email::Email;
pub use engagement::EngagementEventId;
pub use events::EventOutboxId;
pub use execution::{ArtifactId, ExecutionStepId, LogId, TokenId};
pub use gateway_boot::{DepartmentId, DepartmentName, ModelId, ProviderId, RouteId, SecretName};
pub use gateway_conversation::GatewayConversationId;
pub use hook::HookId;
pub use instance::InstanceId;
pub use jobs::{JobName, ScheduledJobId};
pub use links::{CampaignId, LinkClickId, LinkId};
pub use locale::LocaleCode;
pub use managed::{
    ConsumerInstallationId, DistributionId, InstallationReceiptId, InstallationSessionBindingId,
    InventoryEntryId, ManagedReconciliationId, ManagedResourceId, ManagedSourceId, NativeSessionId,
    PublicationId, PublicationReviewId, ResourceRevisionId, SourceSnapshotId, WithdrawalProposalId,
};
pub use marketplace::MarketplaceId;
pub use mcp::{AiToolCallId, McpExecutionId, McpServerId, McpToolName};
pub use oauth::{AccessTokenId, AuthorizationCode, ChallengeId, RefreshTokenId};
pub use path::ValidatedFilePath;
pub use plugin::PluginId;
pub use policy::{CallId, PolicyId, PolicyVersion, SecretPatternId};
pub use profile::ProfileName;
pub use provider_request::ProviderRequestId;
pub use roles::RoleId;
pub use section::SectionId;
pub use service::ServiceName;
pub use session::{SessionId, SessionSource};
pub use slack::{SlackChannelId, SlackUserId, SlackWorkspaceId};
pub use task::TaskId;
pub use teams::{TeamsConversationId, TeamsTenantId, TeamsUserId};
pub use tenant::TenantId;
pub use trace::TraceId;
pub use url::ValidatedUrl;
pub use user::UserId;
pub use webhook::WebhookEndpointId;

define_id!(RuleId, generate);
pub use managed::ResourceInvocationId;
