//! The narrow values a reserved-path command reaches through a kernel grant.
//!
//! A member command reaches kernel-held material only through a grant listed
//! on its reserved CLI path's row. Each grant hands out one purpose-typed
//! value, never the profile, the configuration or a secrets source:
//! [`ServicesTree`] for `KernelGrant::ServicesTree`, [`CallerSession`]
//! for `KernelGrant::CallerSession` and a [`DeploymentSigningKeys`] handle
//! for `KernelGrant::SigningKeys`. The commands `identity-http` owns read
//! the configuration scalars they act on as one [`IdentityCliSettings`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use systemprompt_identifiers::{
    Actor, AgentName, ContextId, JwtToken, SessionId, SessionToken, TraceId, UserId,
};
use systemprompt_manifest::profile::TrustedIssuer;
use systemprompt_models::auth::UserType;
use systemprompt_models::bridge::instance::BridgeVersionWindow;
use systemprompt_models::execution::context::RequestContext;

use super::command::CommandError;

/// The active services tree and the public site values read beside it.
///
/// Every field is a path or a value the profile publishes to clients; none
/// is a secret, and the host builds it from the boot context without a
/// database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicesTree {
    root: PathBuf,
    api_external_url: String,
    site_github_link: Option<String>,
}

impl ServicesTree {
    #[must_use]
    pub const fn new(
        root: PathBuf,
        api_external_url: String,
        site_github_link: Option<String>,
    ) -> Self {
        Self {
            root,
            api_external_url,
            site_github_link,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn plugins(&self) -> PathBuf {
        self.root.join("plugins")
    }

    #[must_use]
    pub fn skills(&self) -> PathBuf {
        self.root.join("skills")
    }

    #[must_use]
    pub fn api_external_url(&self) -> &str {
        &self.api_external_url
    }

    #[must_use]
    pub fn site_github_link(&self) -> Option<&str> {
        self.site_github_link.as_deref()
    }
}

/// The session of the person running the CLI: its token and the caller's own
/// session, user and context ids.
///
/// The host resolves it through the same path the kernel's own commands use;
/// it is always the caller's own session and is never minted for another
/// subject. A request a command makes with it is attributed to that caller,
/// never to an anonymous or shared identity. The token is a platform
/// credential: a command presents it only to an endpoint the platform hosts.
#[derive(Debug, Clone)]
pub struct CallerSession {
    token: SessionToken,
    session_id: SessionId,
    user_id: UserId,
    context_id: ContextId,
    user_type: UserType,
}

impl CallerSession {
    #[must_use]
    pub const fn new(
        token: SessionToken,
        session_id: SessionId,
        user_id: UserId,
        context_id: ContextId,
        user_type: UserType,
    ) -> Self {
        Self {
            token,
            session_id,
            user_id,
            context_id,
            user_type,
        }
    }

    #[must_use]
    pub const fn token(&self) -> &SessionToken {
        &self.token
    }

    #[must_use]
    pub const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    #[must_use]
    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    #[must_use]
    pub const fn context_id(&self) -> &ContextId {
        &self.context_id
    }

    #[must_use]
    pub const fn user_type(&self) -> UserType {
        self.user_type
    }

    #[must_use]
    pub fn request_context(&self, agent_name: AgentName) -> RequestContext {
        RequestContext::new(
            self.session_id.clone(),
            TraceId::generate(),
            self.context_id.clone(),
            agent_name,
            Actor::user(self.user_id.clone()),
        )
        .with_auth_token(JwtToken::new(self.token.as_str()))
        .with_user_type(self.user_type)
    }
}

/// The deployment's signing state, read and rotated by the kernel.
///
/// A `KernelGrant::SigningKeys` holder reads the token signing key ids and
/// the bridge manifest public key, stages, promotes and rotates the token
/// key, and rotates the manifest signing seed. The kernel reads and writes the
/// active profile's secrets file and applies the profile's token lifetimes to
/// a promotion's overlap; the handle never returns a private key, a seed, the
/// profile or the secrets source, and mints no token.
pub trait DeploymentSigningKeys: Send + Sync {
    fn token_keys(&self) -> Result<TokenKeySet, CommandError>;
    fn stage_token_key(&self) -> Result<TokenKeySet, CommandError>;
    fn promote_token_key(&self, promotion: TokenKeyPromotion) -> Result<TokenKeySet, CommandError>;
    fn rotate_token_key(&self, promotion: TokenKeyPromotion) -> Result<TokenKeySet, CommandError>;
    fn manifest_public_key(&self) -> Result<String, CommandError>;
    fn rotate_manifest_key(&self) -> Result<String, CommandError>;
}

/// How a staged token key is promoted: with the outgoing key verifying for an
/// overlap (the requested seconds, or the longest signed-token lifetime), or
/// revoked with no overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKeyPromotion {
    Overlap { secs: Option<i64>, force: bool },
    Revoke,
}

/// The token signing key ids after a read or a rotation step, and the kinds
/// of signed token a forced short overlap cuts off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenKeySet {
    pub active_kid: String,
    pub staged_kid: Option<String>,
    pub retired: Vec<RetiredTokenKey>,
    pub cut_off: Vec<String>,
}

/// A retired token key's id and the instant it stops verifying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetiredTokenKey {
    pub kid: String,
    pub expires_at: DateTime<Utc>,
}

/// The configuration scalars the identity commands act on, built by the
/// kernel from the profile's configuration, its security section and the
/// services tree.
///
/// The host serves it only to a command whose owner is `identity-http` with
/// core standing; nothing in it is a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityCliSettings {
    pub api_key_max_lifetime_days: Option<u32>,
    pub archived_users_days: u32,
    pub api_external_url: String,
    pub bridge_release_window: BridgeVersionWindow,
    pub issuer: String,
    pub trusted_issuers: Vec<TrustedIssuer>,
}
