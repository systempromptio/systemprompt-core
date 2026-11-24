//! [`PluginTokenMinter`]: the hook-credential mint `admin keys
//! issue-plugin-token` runs over.
//!
//! The oauth member provides it **sealed** and confers it on `identity-http`
//! alone (the `identity-plugin-tokens` row), so only that member's commands
//! and route state read it; an install command reads nothing. It mints one
//! shape of token and no other: an RS256 JWT with the `hook` audience plus the
//! `plugin` resource audience, the `HookGovern` and `HookTrack` permissions
//! and the plugin id as its `plugin_id` claim, for one to 365 days. It mints
//! only for an existing, active user holding the admin role, and
//! each token is backed by a fresh CLI session row of the same lifetime.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{AccessTokenId, PluginId, UserId};

use crate::BoxedSource;

pub const PLUGIN_TOKEN_MAX_DAYS: u32 = 365;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginTokenHolder {
    Email(String),
    User(UserId),
}

impl fmt::Display for PluginTokenHolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Email(email) => f.write_str(email),
            Self::User(id) => f.write_str(id.as_str()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluginTokenRequest {
    pub holder: PluginTokenHolder,
    pub plugin_id: PluginId,
    pub duration_days: u32,
}

#[derive(Debug, Clone)]
pub struct MintedPluginToken {
    pub user_id: UserId,
    pub email: String,
    pub jti: AccessTokenId,
    pub token: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginTokenMintError {
    #[error("--duration-days must be between 1 and {PLUGIN_TOKEN_MAX_DAYS} (got {0})")]
    Duration(u32),
    #[error("user '{holder}' not found")]
    UserNotFound { holder: PluginTokenHolder },
    #[error("user '{email}' is not an active admin — refusing to mint")]
    NotAdmin { email: String },
    #[error("no token authority is available on this host")]
    Unavailable,
    #[error("the plugin token could not be minted")]
    Failed(#[source] BoxedSource),
}

/// Mints plugin-scoped hook credentials for admin users. `#[async_trait]`
/// keeps the trait `dyn`-compatible for [`DynPluginTokenMinter`].
#[async_trait]
pub trait PluginTokenMinter: Send + Sync {
    async fn mint(
        &self,
        request: PluginTokenRequest,
    ) -> Result<MintedPluginToken, PluginTokenMintError>;
}

pub type DynPluginTokenMinter = Arc<dyn PluginTokenMinter>;
