//! [`SessionIssuer`]: the browser-session issuance the API's session
//! middleware and static-content serving read, and nothing else.
//!
//! The oauth member provides it **sealed** (it creates sessions and mints
//! session tokens for any user it is handed), over the users member's sealed
//! session store and user directory and the kernel's token authority, so only
//! kernel code reads it. On a host with no token authority (a child host) the
//! provided issuer refuses every call. With no oauth member admitted there is
//! no issuer: the session middleware serves each request with an untracked,
//! actor-less context, which every gate above `public` refuses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{ClientId, JwtToken, SessionId, SessionSource, UserId};

use crate::BoxedSource;
use crate::analytics::SessionAnalytics;

#[derive(Debug, Clone)]
pub struct IssuedSession {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub is_new: bool,
    pub jwt_token: JwtToken,
    pub fingerprint_hash: String,
}

#[derive(Debug, Clone)]
pub struct BrowserSession {
    pub session_id: SessionId,
    pub user_id: UserId,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionIssueError {
    #[error("user not found: {user_id}")]
    UserNotFound { user_id: UserId },
    #[error("no token authority is available on this host")]
    Unavailable,
    #[error("the session could not be issued")]
    Failed(#[source] BoxedSource),
}

/// Browser-session issuance. `#[async_trait]` keeps the trait
/// `dyn`-compatible.
#[async_trait]
pub trait SessionIssuer: Send + Sync {
    async fn anonymous_user(
        &self,
        analytics: &SessionAnalytics,
    ) -> Result<(UserId, String), SessionIssueError>;

    async fn issue_anonymous(
        &self,
        analytics: &SessionAnalytics,
        client_id: &ClientId,
        source: SessionSource,
    ) -> Result<IssuedSession, SessionIssueError>;

    async fn issue_for_user(
        &self,
        user_id: &UserId,
        analytics: &SessionAnalytics,
        source: SessionSource,
    ) -> Result<SessionId, SessionIssueError>;

    fn mint_anonymous_jwt(
        &self,
        user_id: &UserId,
        session_id: &SessionId,
        client_id: &ClientId,
    ) -> Result<JwtToken, SessionIssueError>;

    fn verify_browser_token(&self, token: &str) -> Option<BrowserSession>;

    async fn attest(
        &self,
        session_id: &SessionId,
        user_id: &UserId,
        route: &str,
    ) -> Result<(), SessionIssueError>;
}

pub type DynSessionIssuer = Arc<dyn SessionIssuer>;
