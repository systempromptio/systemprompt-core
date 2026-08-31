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

use axum::extract::{ConnectInfo, Request};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use ipnet::IpNet;
use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_analytics::{AnalyticsService, SessionAnalytics};
use systemprompt_models::api::ApiError;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_runtime::AppContext;
use systemprompt_security::{CookieExtractor, HeaderExtractor};
use systemprompt_traits::{AnalyticsProvider, ExtractSignals};
use systemprompt_users::UserService;

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
}
