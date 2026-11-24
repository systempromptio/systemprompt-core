//! The oauth member's composed provides: the sealed browser-session issuer
//! (`DynSessionIssuer`) the API's session middleware and static-content
//! serving read. It is built over the users member's sealed session store and
//! user directory and the kernel's sealed token authority, all read by this
//! core member's provide fn; on a host that seeds no token authority, or with
//! the users member's seams absent (the member's declared requirement then
//! refuses the composition), it is the issuer that refuses every call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_identifiers::{ClientId, JwtToken, SessionId, SessionSource, UserId};
use systemprompt_manifest::Config;
use systemprompt_models::auth::TokenAuthority;
use systemprompt_traits::{
    AtRestPepper, BrowserSession, DynSessionIssuer, DynSessionStore, DynUserProvider,
    IssuedSession, SessionAnalytics, SessionIssueError, SessionIssuer, SessionProvider,
};

use crate::repository::OAuthRepositories;
use crate::services::generation::generate_anonymous_jwt_with_expiry;
use crate::services::{CreateAnonymousSessionInput, SessionCreationError, SessionCreationService};

struct OauthSessionIssuer {
    service: SessionCreationService,
    sessions: Arc<dyn SessionProvider>,
    authority: Arc<dyn TokenAuthority>,
    config: Arc<Config>,
}

fn failed(e: impl std::error::Error + Send + Sync + 'static) -> SessionIssueError {
    SessionIssueError::Failed(Box::new(e))
}

#[async_trait]
impl SessionIssuer for OauthSessionIssuer {
    async fn anonymous_user(
        &self,
        analytics: &SessionAnalytics,
    ) -> Result<(UserId, String), SessionIssueError> {
        self.service
            .ensure_anonymous_user(analytics)
            .await
            .map_err(failed)
    }

    async fn issue_anonymous(
        &self,
        analytics: &SessionAnalytics,
        client_id: &ClientId,
        source: SessionSource,
    ) -> Result<IssuedSession, SessionIssueError> {
        let info = self
            .service
            .create_anonymous_session(CreateAnonymousSessionInput {
                analytics,
                client_id,
                session_source: source,
            })
            .await
            .map_err(failed)?;
        Ok(IssuedSession {
            session_id: info.session_id,
            user_id: info.user_id,
            is_new: info.is_new,
            jwt_token: info.jwt_token,
            fingerprint_hash: info.fingerprint_hash,
        })
    }

    async fn issue_for_user(
        &self,
        user_id: &UserId,
        analytics: &SessionAnalytics,
        source: SessionSource,
    ) -> Result<SessionId, SessionIssueError> {
        self.service
            .create_authenticated_session(user_id, analytics, source)
            .await
            .map_err(|e| match e {
                SessionCreationError::UserNotFound { user_id } => {
                    SessionIssueError::UserNotFound { user_id }
                },
                SessionCreationError::Internal(source) => SessionIssueError::Failed(source),
            })
    }

    fn mint_anonymous_jwt(
        &self,
        user_id: &UserId,
        session_id: &SessionId,
        client_id: &ClientId,
    ) -> Result<JwtToken, SessionIssueError> {
        generate_anonymous_jwt_with_expiry(
            self.authority.as_ref(),
            user_id,
            session_id,
            client_id,
            self.config.jwt_access_token_expiration,
        )
        .map(JwtToken::new)
        .map_err(failed)
    }

    fn verify_browser_token(&self, token: &str) -> Option<BrowserSession> {
        let claims = self
            .authority
            .verify_bearer(token, &self.config.jwt_audiences)
            .ok()?;
        let session_id = claims.custom.session_id?;
        let user_id = UserId::try_new(claims.registered.sub).ok()?;
        Some(BrowserSession {
            session_id: SessionId::new(session_id),
            user_id,
        })
    }

    async fn attest(
        &self,
        session_id: &SessionId,
        user_id: &UserId,
        route: &str,
    ) -> Result<(), SessionIssueError> {
        systemprompt_security::attest_session(&self.sessions, session_id, user_id, route)
            .await
            .map_err(failed)
    }
}

struct UnavailableSessionIssuer;

#[async_trait]
impl SessionIssuer for UnavailableSessionIssuer {
    async fn anonymous_user(
        &self,
        _analytics: &SessionAnalytics,
    ) -> Result<(UserId, String), SessionIssueError> {
        Err(SessionIssueError::Unavailable)
    }

    async fn issue_anonymous(
        &self,
        _analytics: &SessionAnalytics,
        _client_id: &ClientId,
        _source: SessionSource,
    ) -> Result<IssuedSession, SessionIssueError> {
        Err(SessionIssueError::Unavailable)
    }

    async fn issue_for_user(
        &self,
        _user_id: &UserId,
        _analytics: &SessionAnalytics,
        _source: SessionSource,
    ) -> Result<SessionId, SessionIssueError> {
        Err(SessionIssueError::Unavailable)
    }

    fn mint_anonymous_jwt(
        &self,
        _user_id: &UserId,
        _session_id: &SessionId,
        _client_id: &ClientId,
    ) -> Result<JwtToken, SessionIssueError> {
        Err(SessionIssueError::Unavailable)
    }

    fn verify_browser_token(&self, _token: &str) -> Option<BrowserSession> {
        None
    }

    async fn attest(
        &self,
        _session_id: &SessionId,
        _user_id: &UserId,
        _route: &str,
    ) -> Result<(), SessionIssueError> {
        Err(SessionIssueError::Unavailable)
    }
}

pub(crate) fn session_issuer(r: &Resolved<'_>) -> Result<DynSessionIssuer, ProvideError> {
    let (Some(authority), Some(store), Some(users)) = (
        r.optional::<Arc<dyn TokenAuthority>>(),
        r.optional::<DynSessionStore>(),
        r.optional::<DynUserProvider>(),
    ) else {
        return Ok(Arc::new(UnavailableSessionIssuer));
    };
    let sessions: Arc<dyn SessionProvider> = store;
    let config = r.get::<Arc<Config>>()?;
    Ok(Arc::new(OauthSessionIssuer {
        service: SessionCreationService::new(
            Arc::clone(&sessions),
            users,
            Arc::clone(&authority),
            Arc::clone(&config),
        ),
        sessions,
        authority,
        config,
    }))
}

pub(crate) fn oauth_repositories(r: &Resolved<'_>) -> Result<Arc<OAuthRepositories>, ProvideError> {
    let Some(pepper) = r.optional::<AtRestPepper>() else {
        return Err(ProvideError::Withheld);
    };
    let db = r.get::<DbPool>()?;
    Ok(Arc::new(OAuthRepositories::new(&db, pepper.shared())))
}
