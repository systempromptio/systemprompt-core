//! Session-establishment middleware.
//!
//! [`SessionMiddleware`] resolves or mints the per-request session: it skips
//! untracked paths, short-circuits detected bots into anonymous contexts,
//! validates an existing JWT session, and refreshes or recreates the session
//! when the token is stale, issuing a `Set-Cookie` for newly minted tokens.
//!
//! Validation runs through [`attest_session`], the same predicate the JWT and
//! gateway credential paths use: a cookie must name a session the server issued
//! *to that user*. An existence-only check would let a signed token borrow
//! another user's live session for analytics attribution.
//!
//! The session store is the one the `users` extension provides in the
//! extension state, read through [`systemprompt_traits::SessionProvider`];
//! request analytics and the anonymous-user provider are read through their
//! shared traits. When any of them is absent the middleware is not built and
//! boot refuses with a typed `LoaderError::MissingProvider`: a session is
//! never minted without the request analytics that attribute it.
//!
//! Establishing a session reads and writes the database, so it is bounded and
//! degrades rather than fails — see the `context` submodule. A request that
//! cannot be given a session is served with an untracked, actor-less context
//! instead of a 500: the alternative is that a database fault takes the public
//! site down, and a page view is worth more than the analytics row describing
//! it. Nothing is escalated by the degraded context — it carries no auth token
//! and no user, so every gate above `public` still refuses it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod attestation;
mod context;
mod lifecycle;
mod skip;

pub use attestation::{SessionAttestationError, attest_session};
pub use skip::should_skip_session_tracking;

use crate::error::ApiHttpError;
use crate::services::session_cookie::{SameSite, SessionCookie};
use axum::extract::{ConnectInfo, Request};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use ipnet::IpNet;
use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::Config;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_runtime::AppContext;
use systemprompt_security::HeaderExtractor;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{
    AnalyticsProvider, AppContext as _, ExtractSignals, SessionAnalytics, SessionProvider,
    UserProvider,
};

struct RequestMeta<'a> {
    headers: &'a http::HeaderMap,
    uri: &'a http::Uri,
    analytics: &'a SessionAnalytics,
}

#[derive(Clone)]
pub struct SessionMiddleware {
    analytics: Arc<dyn AnalyticsProvider>,
    sessions: Arc<dyn SessionProvider>,
    session_creation_service: Arc<SessionCreationService>,
    trusted_proxies: Arc<Vec<IpNet>>,
    config: Arc<Config>,
    signing_keys: SigningKeys,
    ignored_forwarded_warn: Arc<systemprompt_logging::LogThrottle>,
    degraded_warn: Arc<systemprompt_logging::LogThrottle>,
}

pub(crate) type SessionProviders = (
    Arc<dyn AnalyticsProvider>,
    Arc<dyn SessionProvider>,
    SessionCreationService,
);

pub(crate) fn session_providers(ctx: &AppContext) -> Result<SessionProviders, LoaderError> {
    let consumer = || ExtensionId::new("session");
    let analytics = ctx
        .analytics_provider()
        .ok_or_else(|| LoaderError::missing_provider::<Arc<dyn AnalyticsProvider>>(consumer()))?;
    let sessions = ctx
        .session_provider()
        .ok_or_else(|| LoaderError::missing_provider::<Arc<dyn SessionProvider>>(consumer()))?;
    let users = ctx
        .user_provider()
        .ok_or_else(|| LoaderError::missing_provider::<Arc<dyn UserProvider>>(consumer()))?;
    let creation = SessionCreationService::new(
        Arc::clone(&sessions),
        users,
        ctx.token_authority(),
        Arc::clone(ctx.config_arc()),
    );
    Ok((analytics, sessions, creation))
}

const IGNORED_FORWARDED_WARN_INTERVAL_SECS: u64 = 3600;
const DEGRADED_WARN_INTERVAL_SECS: u64 = 60;
const SESSION_COOKIE_MAX_AGE_SECS: i64 = 604_800;

impl std::fmt::Debug for SessionMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionMiddleware")
            .field("trusted_proxies", &self.trusted_proxies)
            .finish_non_exhaustive()
    }
}

impl SessionMiddleware {
    pub fn new(ctx: &AppContext) -> Result<Self, LoaderError> {
        let (analytics, sessions, session_creation_service) = session_providers(ctx)?;

        Ok(Self {
            analytics,
            sessions,
            session_creation_service: Arc::new(session_creation_service),
            trusted_proxies: Arc::new(ctx.config().trusted_proxies.clone()),
            config: Arc::clone(ctx.config_arc()),
            signing_keys: ctx.signing_keys().clone(),
            ignored_forwarded_warn: Arc::new(systemprompt_logging::LogThrottle::new(
                IGNORED_FORWARDED_WARN_INTERVAL_SECS,
            )),
            degraded_warn: Arc::new(systemprompt_logging::LogThrottle::new(
                DEGRADED_WARN_INTERVAL_SECS,
            )),
        })
    }

    pub async fn handle(&self, mut request: Request, next: Next) -> Result<Response, ApiHttpError> {
        let caller_ip = super::client_addr::resolve_client_ip(
            request.headers(),
            request.extensions().get::<ConnectInfo<SocketAddr>>(),
            &self.trusted_proxies,
        );
        if let Some(peer) = request.extensions().get::<ConnectInfo<SocketAddr>>()
            && super::client_addr::forwarded_headers_ignored(
                request.headers(),
                peer.0.ip(),
                &self.trusted_proxies,
            )
            && self.ignored_forwarded_warn.allow()
        {
            tracing::warn!(
                peer_ip = %peer.0.ip(),
                "ignoring forwarded client-IP headers from an untrusted peer; if this \
                 server runs behind a proxy, add the peer's range to server.trusted_proxies"
            );
        }
        let uri = request.uri().clone();
        let headers = request.headers();
        let analytics = self.analytics.extract_analytics(
            headers,
            ExtractSignals {
                uri: Some(&uri),
                caller_ip,
            },
        );
        let meta = RequestMeta {
            headers,
            uri: &uri,
            analytics: &analytics,
        };

        let should_skip = should_skip_session_tracking(uri.path());

        tracing::debug!(
            path = %uri.path(),
            should_skip = should_skip,
            "Session middleware evaluating request"
        );

        let trace_id = HeaderExtractor::extract_trace_id(headers);

        let (req_ctx, jwt_cookie) = self
            .establish_or_degrade(should_skip, trace_id, &meta, uri.path())
            .await;

        tracing::debug!(
            path = %uri.path(),
            session_id = %req_ctx.session_id(),
            "Session middleware setting context"
        );

        request.extensions_mut().insert(req_ctx);

        let mut response = next.run(request).await;

        if let Some(token) = jwt_cookie
            && let Ok(cookie_value) = SessionCookie::from_config(&self.config).issue(
                &token,
                SESSION_COOKIE_MAX_AGE_SECS,
                SameSite::Strict,
            )
        {
            response
                .headers_mut()
                .insert(header::SET_COOKIE, cookie_value);
        }

        Ok(response)
    }
}
