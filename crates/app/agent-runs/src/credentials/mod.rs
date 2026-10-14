//! Ephemeral credentials issued to one run.
//!
//! Every run gets its own PAT, a gateway session and, when the host supports
//! hooks, a hook token, and when it carries a purpose an MCP token, so every
//! gateway call the host makes is attributed to the run's user and revoked when
//! the run ends. [`RunCredentialService`] issues and revokes them in-process,
//! with no HTTP round trip to the gateway.
//!
//! `IssueRunCredentials::purpose` is the caller purpose bound to the PAT by its
//! issuer (`farm:<run_id>`, `workflow:<id>`); the broker reads it from the
//! credential, never from a client header. `None` issues an unpurposed key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod hook_token;
pub mod mcp_token;
pub mod pat;
pub mod session;

mod service;

pub use service::RunCredentialService;

use std::time::Duration;

use chrono::{DateTime, Utc};

use systemprompt_identifiers::{
    AccessTokenId, ApiKeyId, ApiKeySecret, PluginId, SessionId, UserId,
};
use systemprompt_models::attribution::ScopeBinding;

/// The credentials one run holds.
#[derive(Debug, Clone)]
pub struct RunCredentials {
    pub user_id: UserId,
    pub api_key_id: ApiKeyId,
    pub pat: ApiKeySecret,
    pub gateway_session_id: SessionId,
    pub hook_token: Option<String>,
    pub hook_token_jti: Option<AccessTokenId>,
    pub hook_token_expires_at: Option<DateTime<Utc>>,
    pub mcp_token: Option<String>,
    pub mcp_token_jti: Option<AccessTokenId>,
    pub mcp_token_expires_at: Option<DateTime<Utc>>,
}

/// Spend and rate ceilings carried by a run's credentials.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RunCredentialLimits {
    pub max_budget_usd: Option<f64>,
    pub max_requests: Option<u32>,
}

/// What to issue for a run.
#[derive(Debug, Clone)]
pub struct IssueRunCredentials {
    pub user_id: UserId,
    pub name: String,
    pub ttl: Duration,
    pub scopes: Vec<ScopeBinding>,
    pub plugin_id: Option<PluginId>,
    pub limits: RunCredentialLimits,
    pub purpose: Option<String>,
}
