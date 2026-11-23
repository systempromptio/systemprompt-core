//! [`JwtRequestAuthenticator`], the one implementation of
//! [`RequestAuthenticator`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderMap;
use systemprompt_identifiers::{ContextId, JwtToken, UserId, headers as sp_headers};
use systemprompt_models::auth::{
    DynTokenRevocations, JwtAudience, JwtUserContext, RequestAuthenticator,
};
use systemprompt_models::execution::context::{ContextExtractionError, RequestContext};
use systemprompt_traits::{AuthUser, SessionProvider, UserProvider};

use super::context::{
    BuildContextParams, build_context, extract_common_headers, is_cookie_transport,
};
use super::users::{UserCache, user_is_admin, validate_session_exists, validate_user_exists};
use crate::extraction::TokenExtractor;
use crate::jwt::{ValidationPolicy, extract_user_context, extract_user_context_for};
use crate::keys::VerificationKeyring;

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("a ui_session token is a bearer credential and is refused as the session cookie")]
struct UiSessionCookieRefused;

/// What a [`JwtRequestAuthenticator`] is built from.
pub struct JwtRequestAuthenticatorParts {
    pub sessions: Arc<dyn SessionProvider>,
    pub users: Arc<dyn UserProvider>,
    pub revocations: DynTokenRevocations,
    pub issuer: String,
    pub keyring: VerificationKeyring,
}

impl std::fmt::Debug for JwtRequestAuthenticatorParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtRequestAuthenticatorParts")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

/// The kernel's request authenticator over signed session tokens.
#[derive(Clone)]
pub struct JwtRequestAuthenticator {
    token_extractor: TokenExtractor,
    sessions: Arc<dyn SessionProvider>,
    users: Arc<dyn UserProvider>,
    user_cache: Arc<UserCache>,
    revocations: DynTokenRevocations,
    issuer: String,
    keyring: VerificationKeyring,
}

impl std::fmt::Debug for JwtRequestAuthenticator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtRequestAuthenticator")
            .field("token_extractor", &self.token_extractor)
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl JwtRequestAuthenticator {
    #[must_use]
    pub fn new(parts: JwtRequestAuthenticatorParts) -> Self {
        Self {
            token_extractor: TokenExtractor::browser_only(),
            sessions: parts.sessions,
            users: parts.users,
            user_cache: UserCache::new(),
            revocations: parts.revocations,
            issuer: parts.issuer,
            keyring: parts.keyring,
        }
    }

    fn verified_header_claims(
        &self,
        headers: &HeaderMap,
        audiences: &[JwtAudience],
    ) -> Result<JwtUserContext, ContextExtractionError> {
        let token = self
            .token_extractor
            .extract(headers)
            .map_err(|_e| ContextExtractionError::MissingAuthHeader)?;
        let policy = ValidationPolicy::issuer_scoped(&self.issuer, audiences);
        extract_user_context_for(&self.keyring, &token, &policy)
            .map_err(|e| ContextExtractionError::InvalidToken(e.into()))
    }

    async fn validate(
        &self,
        jwt_context: &JwtUserContext,
        route_context: &str,
    ) -> Result<AuthUser, ContextExtractionError> {
        if jwt_context.session_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingSessionId);
        }
        if jwt_context.user_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingUserId);
        }
        let validated = validate_user_exists(
            &self.users,
            &self.user_cache,
            &jwt_context.user_id,
            route_context,
        )
        .await?;
        validate_session_exists(&self.sessions, jwt_context, route_context).await?;
        if let Some(jti) = &jwt_context.jti {
            self.revocations.ensure_not_revoked(jti).await?;
        }
        Ok(validated.user)
    }

    fn context(
        &self,
        headers: &HeaderMap,
        jwt_context: JwtUserContext,
        user: &AuthUser,
        context_id: ContextId,
    ) -> RequestContext {
        let (trace_id, header_task, auth_token, agent_name) =
            extract_common_headers(&self.token_extractor, headers);
        let user_type = jwt_context.user_type.reconcile_with(user_is_admin(user));
        let session_id = jwt_context.session_id.clone();
        let user_id = jwt_context.user_id.clone();
        build_context(BuildContextParams {
            jwt_context,
            session_id,
            user_id,
            trace_id,
            context_id,
            agent_name,
            task_id: header_task,
            auth_token,
            user_type,
            cookie_transport: is_cookie_transport(&self.token_extractor, headers),
        })
    }
}

#[async_trait]
impl RequestAuthenticator for JwtRequestAuthenticator {
    async fn authenticate(
        &self,
        headers: &HeaderMap,
        audiences: &[JwtAudience],
    ) -> Result<RequestContext, ContextExtractionError> {
        let jwt_context = self.verified_header_claims(headers, audiences)?;
        if jwt_context.ui_session && is_cookie_transport(&self.token_extractor, headers) {
            return Err(ContextExtractionError::InvalidToken(Box::new(
                UiSessionCookieRefused,
            )));
        }
        let user = self.validate(&jwt_context, "").await?;
        let context_id = headers
            .get(sp_headers::CONTEXT_ID)
            .and_then(|h| h.to_str().ok())
            .filter(|s| !s.is_empty())
            .and_then(|s| ContextId::try_new(s).ok())
            .unwrap_or_else(|| ContextId::derived_from_session(&jwt_context.session_id));
        Ok(self.context(headers, jwt_context, &user, context_id))
    }

    async fn decode_for_audiences(
        &self,
        token: &JwtToken,
        audiences: &[JwtAudience],
    ) -> Result<(JwtUserContext, AuthUser), ContextExtractionError> {
        let policy = ValidationPolicy::issuer_scoped(&self.issuer, audiences);
        let jwt_context = extract_user_context_for(&self.keyring, token.as_str(), &policy)
            .map_err(|e| ContextExtractionError::InvalidToken(e.into()))?;
        let user = self.validate(&jwt_context, "drive sync").await?;
        Ok((jwt_context, user))
    }

    async fn decode_for_gateway(
        &self,
        token: &JwtToken,
    ) -> Result<(JwtUserContext, AuthUser), ContextExtractionError> {
        let jwt_context = extract_user_context(&self.keyring, token.as_str(), &self.issuer)
            .map_err(|e| ContextExtractionError::InvalidToken(e.into()))?;
        let user = self.validate(&jwt_context, "gateway").await?;
        Ok((jwt_context, user))
    }

    async fn load_user(
        &self,
        user_id: &UserId,
        route_context: &str,
    ) -> Result<AuthUser, ContextExtractionError> {
        validate_user_exists(&self.users, &self.user_cache, user_id, route_context)
            .await
            .map(|validated| validated.user)
    }

    fn verified_claims(&self, token: &str) -> Option<JwtUserContext> {
        extract_user_context(&self.keyring, token, &self.issuer).ok()
    }
}
