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
//! Establishing a session reads and writes the database, so it is bounded by
//! [`SESSION_ESTABLISH_TIMEOUT`] and degrades rather than fails. A request that
//! cannot be given a session is served with an untracked, actor-less context
//! instead of a 500: the alternative is that a database fault takes the public
//! site down, and a page view is worth more than the analytics row describing
//! it. Nothing is escalated by the degraded context — it carries no auth token
//! and no user, so every gate above `public` still refuses it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod attestation;
mod lifecycle;
mod skip;

pub use attestation::{SessionAttestationError, attest_session};
pub use skip::should_skip_session_tracking;

use axum::extract::{ConnectInfo, Request};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use ipnet::IpNet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_analytics::{AnalyticsService, SessionAnalytics};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, UserId};
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_runtime::AppContext;
use systemprompt_security::{
    CookieExtractor, HeaderExtractor, TokenExtractor, extract_user_context,
};
use systemprompt_traits::{AnalyticsProvider, ExtractSignals};
use systemprompt_users::UserService;
use uuid::Uuid;

struct RequestMeta<'a> {
    headers: &'a http::HeaderMap,
    uri: &'a http::Uri,
    analytics: &'a SessionAnalytics,
}

#[derive(Clone, Debug)]
pub struct SessionMiddleware {
    analytics_service: Arc<AnalyticsService>,
    session_creation_service: Arc<SessionCreationService>,
    trusted_proxies: Arc<Vec<IpNet>>,
    ignored_forwarded_warn: Arc<systemprompt_logging::LogThrottle>,
    degraded_warn: Arc<systemprompt_logging::LogThrottle>,
}

const IGNORED_FORWARDED_WARN_INTERVAL_SECS: u64 = 3600;
const DEGRADED_WARN_INTERVAL_SECS: u64 = 60;

// Why: the pool's own acquire timeout is 30s, which is a page load nobody
// waits for and, per connection a browser opens, a site that reads as hung
// rather than degraded. A healthy anonymous-user lookup is single-digit
// milliseconds, so this leaves a hundredfold margin before we give up on it.
const SESSION_ESTABLISH_TIMEOUT: Duration = Duration::from_secs(2);

impl SessionMiddleware {
    pub fn new(ctx: &AppContext) -> Self {
        let user_service = UserService::new(Arc::clone(ctx.user_repository()));
        let concrete = Arc::clone(ctx.analytics_service());
        let analytics: Arc<dyn AnalyticsProvider> = concrete;
        let session_creation_service = Arc::new(SessionCreationService::new(
            analytics,
            Arc::new(user_service),
        ));

        Self {
            analytics_service: Arc::clone(ctx.analytics_service()),
            session_creation_service,
            trusted_proxies: Arc::new(ctx.config().trusted_proxies.clone()),
            ignored_forwarded_warn: Arc::new(systemprompt_logging::LogThrottle::new(
                IGNORED_FORWARDED_WARN_INTERVAL_SECS,
            )),
            degraded_warn: Arc::new(systemprompt_logging::LogThrottle::new(
                DEGRADED_WARN_INTERVAL_SECS,
            )),
        }
    }

    pub async fn handle(&self, mut request: Request, next: Next) -> Result<Response, ApiError> {
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
                "ignoring forwarded client-IP headers from untrusted private peer; if this \
                 server runs behind a proxy, add the peer's range to server.trusted_proxies"
            );
        }
        let uri = request.uri().clone();
        let headers = request.headers();
        let analytics = self.analytics_service.extract_analytics(
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

        if let Some(token) = jwt_cookie {
            let cookie = format!(
                "{}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=604800",
                CookieExtractor::DEFAULT_COOKIE_NAME
            );
            if let Ok(cookie_value) = cookie.parse() {
                response
                    .headers_mut()
                    .insert(header::SET_COOKIE, cookie_value);
            }
        }

        Ok(response)
    }

    async fn establish_or_degrade(
        &self,
        should_skip: bool,
        trace_id: systemprompt_identifiers::TraceId,
        meta: &RequestMeta<'_>,
        path: &str,
    ) -> (RequestContext, Option<String>) {
        let establish = async {
            if should_skip {
                Ok((
                    self.anonymous_context("untracked", trace_id.clone(), meta)
                        .await?,
                    None,
                ))
            } else {
                self.tracked_context(trace_id.clone(), meta).await
            }
        };

        match tokio::time::timeout(SESSION_ESTABLISH_TIMEOUT, establish).await {
            Ok(Ok(established)) => established,
            Ok(Err(e)) => {
                self.warn_degraded(path, &e.message);
                (Self::degraded_context(trace_id), None)
            },
            Err(_) => {
                self.warn_degraded(
                    path,
                    "timed out establishing a session; the database did not answer",
                );
                (Self::degraded_context(trace_id), None)
            },
        }
    }

    fn warn_degraded(&self, path: &str, reason: &str) {
        if self.degraded_warn.allow() {
            tracing::warn!(
                path,
                reason,
                "serving without a session; page views are not being attributed until the \
                 database recovers"
            );
        }
    }

    // Why: no actor and no auth token, so the `unset` user this leaves in
    // place cannot be mistaken for an identity and every gate above `public`
    // still refuses the request. `is_tracked` is false, which is what keeps
    // the analytics sinks from recording a visit they cannot attribute.
    fn degraded_context(trace_id: systemprompt_identifiers::TraceId) -> RequestContext {
        let session_id = SessionId::new(format!("degraded_{}", Uuid::new_v4()));
        let context_id = ContextId::derived_from_session(&session_id);
        RequestContext::new(session_id, trace_id, context_id, AgentName::system())
            .with_user_type(UserType::Anon)
            .with_tracked(false)
    }

    async fn anonymous_context(
        &self,
        session_prefix: &str,
        trace_id: systemprompt_identifiers::TraceId,
        meta: &RequestMeta<'_>,
    ) -> Result<RequestContext, ApiError> {
        let (user_id, fingerprint) = self
            .session_creation_service
            .ensure_anonymous_user(meta.analytics)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, session_prefix, "Failed to ensure anonymous user");
                ApiError::internal_error("Service temporarily unavailable")
            })?;

        let session_id = SessionId::new(format!("{session_prefix}_{}", Uuid::new_v4()));
        let context_id = ContextId::derived_from_session(&session_id);
        Ok(
            RequestContext::new(session_id, trace_id, context_id, AgentName::system())
                .with_actor(systemprompt_identifiers::Actor::anonymous(user_id))
                .with_user_type(UserType::Anon)
                .with_tracked(false)
                .with_fingerprint_hash(fingerprint),
        )
    }

    async fn tracked_context(
        &self,
        trace_id: systemprompt_identifiers::TraceId,
        meta: &RequestMeta<'_>,
    ) -> Result<(RequestContext, Option<String>), ApiError> {
        tracing::debug!(
            path = %meta.uri.path(),
            skip_tracking = meta.analytics.skip_tracking,
            user_agent = ?meta.analytics.user_agent,
            "Session middleware bot check"
        );

        if meta.analytics.skip_tracking {
            return Ok((self.anonymous_context("bot", trace_id, meta).await?, None));
        }

        let token_result = TokenExtractor::browser_only().extract(meta.headers).ok();

        let (session_id, user_id, jwt_token, jwt_cookie, fingerprint_hash) =
            self.resolve_session(token_result, meta).await?;

        let context_id = HeaderExtractor::extract_context_id(meta.headers)
            .unwrap_or_else(|| ContextId::derived_from_session(&session_id));

        let mut ctx = RequestContext::new(session_id, trace_id, context_id, AgentName::system())
            .with_actor(systemprompt_identifiers::Actor::user(user_id))
            .with_auth_token(jwt_token)
            .with_user_type(UserType::Anon)
            .with_tracked(true);
        if let Some(fp) = fingerprint_hash {
            ctx = ctx.with_fingerprint_hash(fp);
        }
        Ok((ctx, jwt_cookie))
    }

    async fn resolve_session(
        &self,
        token_result: Option<String>,
        meta: &RequestMeta<'_>,
    ) -> Result<(SessionId, UserId, String, Option<String>, Option<String>), ApiError> {
        let Some(token) = token_result else {
            let (sid, uid, token, is_new, fp) =
                lifecycle::create_new_session(&self.session_creation_service, meta).await?;
            let jwt_cookie = if is_new { Some(token.clone()) } else { None };
            return Ok((sid, uid, token, jwt_cookie, Some(fp)));
        };

        let Ok(jwt_context) = extract_user_context(&token) else {
            let (sid, uid, token, is_new, fp) =
                lifecycle::create_new_session(&self.session_creation_service, meta).await?;
            let jwt_cookie = if is_new { Some(token.clone()) } else { None };
            return Ok((sid, uid, token, jwt_cookie, Some(fp)));
        };

        let analytics_provider: Arc<dyn AnalyticsProvider> =
            Arc::<AnalyticsService>::clone(&self.analytics_service);

        match attest_session(
            &analytics_provider,
            &jwt_context.session_id,
            &jwt_context.user_id,
            "session_middleware",
        )
        .await
        {
            Ok(()) => {
                return Ok((
                    jwt_context.session_id,
                    jwt_context.user_id,
                    token,
                    None,
                    None,
                ));
            },
            Err(e) => tracing::info!(
                old_session_id = %jwt_context.session_id,
                user_id = %jwt_context.user_id,
                reason = %e,
                "JWT session failed attestation, refreshing with new session"
            ),
        }

        match lifecycle::refresh_session_for_user(
            &self.session_creation_service,
            &jwt_context.user_id,
            meta,
        )
        .await
        {
            Ok((sid, uid, new_token, _, fp)) => {
                Ok((sid, uid, new_token.clone(), Some(new_token), Some(fp)))
            },
            Err(e) if e.error_key.as_deref() == Some("user_not_found") => {
                tracing::warn!(
                    user_id = %jwt_context.user_id,
                    "JWT references non-existent user, creating new anonymous session"
                );
                let (sid, uid, token, _, fp) =
                    lifecycle::create_new_session(&self.session_creation_service, meta).await?;
                Ok((sid, uid, token.clone(), Some(token), Some(fp)))
            },
            Err(e) => Err(e),
        }
    }
}
