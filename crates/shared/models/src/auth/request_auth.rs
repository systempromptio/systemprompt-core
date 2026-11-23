//! The shared request authenticator.
//!
//! [`RequestAuthenticator`] verifies the bearer or session cookie a request
//! carries — signature, issuer, audience, the session row, the user row and
//! the token's revocation — and yields the caller's [`RequestContext`] or the
//! verified claims. It verifies and never mints. One value is built per
//! kernel and read open by route state; every request-context extractor the
//! kernel mounts delegates to it, so a request is verified one way.
//!
//! [`TokenRevocations`] answers the revocation question the authenticator
//! asks last; it fails closed (a store error refuses the token).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use http::HeaderMap;
use systemprompt_identifiers::{AccessTokenId, Actor, ClientId, JwtToken, SessionId, UserId};
use systemprompt_traits::AuthUser;

use super::{JwtAudience, Permission, TokenInteraction, UserType};
use crate::execution::context::{ContextExtractionError, RequestContext};

/// The verified claims of a session-bound bearer the request context is built
/// from.
#[derive(Debug, Clone)]
pub struct JwtUserContext {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub role: Permission,
    pub user_type: UserType,
    pub client_id: Option<ClientId>,
    pub act_chain: Vec<Actor>,
    // JSON: ABAC attribute bag — JWT claim values are policy-defined and schema-less.
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub jti: Option<AccessTokenId>,
    pub exp: i64,
    pub purpose: Option<String>,
    pub interaction: Option<TokenInteraction>,
    pub browser_session: bool,
    pub ui_session: bool,
    pub auth_time: i64,
}

/// Verifies a request and yields the caller; injected as
/// `dyn RequestAuthenticator`, hence `#[async_trait]`.
///
/// `authenticate` reads the context id from the `x-context-id` header (or
/// derives it from the session) and admits the given audiences.
/// `decode_for_audiences` and `decode_for_gateway` verify a presented token and
/// return its claims and user; `verified_claims` checks signature and issuer
/// only, for callers that attest the session themselves.
#[async_trait]
pub trait RequestAuthenticator: Send + Sync + std::fmt::Debug {
    async fn authenticate(
        &self,
        headers: &HeaderMap,
        audiences: &[JwtAudience],
    ) -> Result<RequestContext, ContextExtractionError>;

    async fn decode_for_audiences(
        &self,
        token: &JwtToken,
        audiences: &[JwtAudience],
    ) -> Result<(JwtUserContext, AuthUser), ContextExtractionError>;

    async fn decode_for_gateway(
        &self,
        token: &JwtToken,
    ) -> Result<(JwtUserContext, AuthUser), ContextExtractionError>;

    async fn load_user(
        &self,
        user_id: &UserId,
        route_context: &str,
    ) -> Result<AuthUser, ContextExtractionError>;

    fn verified_claims(&self, token: &str) -> Option<JwtUserContext>;
}

pub type DynRequestAuthenticator = Arc<dyn RequestAuthenticator>;

/// Whether a token id has been revoked; injected as `dyn TokenRevocations`,
/// hence `#[async_trait]`.
#[async_trait]
pub trait TokenRevocations: Send + Sync + std::fmt::Debug {
    async fn ensure_not_revoked(&self, jti: &AccessTokenId) -> Result<(), ContextExtractionError>;
}

pub type DynTokenRevocations = Arc<dyn TokenRevocations>;
