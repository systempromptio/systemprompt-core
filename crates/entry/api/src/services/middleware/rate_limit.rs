//! Router extension traits for rate limiting and authenticated route groups.
//!
//! `RouterExt::with_auth` attaches authentication and authorization in one
//! call: it requires an `AuthzPolicy`, so a route group cannot be mounted
//! authenticated-but-unauthorized — omitting the policy is a compile error.
//!
//! `RouterExt::with_rate_limit` mounts two throttles: the in-process governor
//! keyed by verified identity or trusted client IP, which smooths bursts per
//! replica, and a database-backed window keyed by verified identity only,
//! which bounds a caller's budget across every replica of the deployment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::middleware::authz::{AuthzPolicy, authz_gate};
use crate::services::middleware::client_addr::resolve_client_ip;
use crate::services::middleware::context::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use ipnet::IpNet;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_extension::LoaderError;
use systemprompt_models::auth::UserType;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::{Config, RequestContext};
use systemprompt_runtime::AppContext;
use systemprompt_users::UserRateLimitBucketRepository;

const GLOBAL_WINDOW_SECS: i64 = 10;

#[derive(Clone, Debug)]
pub struct RateLimitState {
    config: RateLimitConfig,
    trusted_proxies: Arc<Vec<IpNet>>,
    buckets: Arc<UserRateLimitBucketRepository>,
}

impl RateLimitState {
    #[must_use]
    pub fn new(config: &Config, buckets: Arc<UserRateLimitBucketRepository>) -> Self {
        Self {
            config: config.rate_limits,
            trusted_proxies: Arc::new(config.trusted_proxies.clone()),
            buckets,
        }
    }

    pub fn from_context(ctx: &AppContext) -> Result<Self, LoaderError> {
        let buckets = crate::repository::user_rate_limit_buckets(ctx.db_pool()).map_err(|e| {
            LoaderError::InitializationFailed {
                extension: "rate_limit".to_owned(),
                message: e.to_string(),
            }
        })?;
        Ok(Self::new(ctx.config(), buckets))
    }
}

#[derive(Clone, Debug)]
struct GlobalUserLimit {
    buckets: Arc<UserRateLimitBucketRepository>,
    scope: &'static str,
    budget: i64,
}

fn window_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let secs = now.timestamp();
    let start = secs - secs.rem_euclid(GLOBAL_WINDOW_SECS);
    DateTime::from_timestamp(start, 0).unwrap_or(now)
}

fn too_many_requests(now: DateTime<Utc>, start: DateTime<Utc>) -> Response {
    let elapsed = now.timestamp() - start.timestamp();
    let retry_after = (GLOBAL_WINDOW_SECS - elapsed).max(1);
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, retry_after.to_string())],
        "rate limit exceeded",
    )
        .into_response()
}

// Why: the governor above already refuses local bursts, so this layer only
// has to bound the sum across replicas. It fails open on a database fault:
// an HTTP throttle protects capacity, not data, and the ban gate ahead of it
// is the one that stays closed.
async fn global_user_rate_limit(
    State(limit): State<GlobalUserLimit>,
    req: Request,
    next: Next,
) -> Response {
    let user_id = req
        .extensions()
        .get::<RequestContext>()
        .filter(|ctx| ctx.auth.user_type != UserType::Anon)
        .map(|ctx| ctx.user_id().clone());
    let Some(user_id) = user_id else {
        return next.run(req).await;
    };

    let now = Utc::now();
    let start = window_start(now);
    match limit.buckets.hit(&user_id, limit.scope, start).await {
        Ok(hits) if hits > limit.budget => {
            tracing::debug!(
                user_id = %user_id,
                scope = limit.scope,
                hits,
                budget = limit.budget,
                "global user rate limit exceeded"
            );
            too_many_requests(now, start)
        },
        Ok(_) => next.run(req).await,
        Err(err) => {
            tracing::warn!(
                user_id = %user_id,
                scope = limit.scope,
                error = %err,
                "global user rate limit unavailable; admitting request"
            );
            next.run(req).await
        },
    }
}

#[derive(Clone, Debug)]
pub struct IdentityOrTrustedIpKey {
    trusted_proxies: Arc<Vec<IpNet>>,
}

impl IdentityOrTrustedIpKey {
    const fn new(trusted_proxies: Arc<Vec<IpNet>>) -> Self {
        Self { trusted_proxies }
    }
}

impl tower_governor::key_extractor::KeyExtractor for IdentityOrTrustedIpKey {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, tower_governor::GovernorError> {
        // Why: an anonymous context's user id is a hash of the User-Agent and
        // Accept-Language headers, so a caller who rotates either would mint a fresh
        // bucket per request. Only a signature-verified identity is safe to key on.
        if let Some(ctx) = req.extensions().get::<RequestContext>()
            && ctx.auth.user_type != UserType::Anon
        {
            return Ok(format!("u:{}", ctx.user_id()));
        }

        resolve_client_ip(
            req.headers(),
            req.extensions().get::<ConnectInfo<SocketAddr>>(),
            &self.trusted_proxies,
        )
        .map(|ip| format!("ip:{ip}"))
        .ok_or(tower_governor::GovernorError::UnableToExtractKey)
    }
}

pub trait ContextLayer: Clone + Send + Sync + 'static {
    fn handle(self, req: Request, next: Next) -> impl Future<Output = Response> + Send;
}

impl ContextLayer for PublicContextMiddleware {
    async fn handle(self, req: Request, next: Next) -> Response {
        Self::handle(&self, req, next).await
    }
}

impl ContextLayer for UserOnlyContextMiddleware {
    async fn handle(self, req: Request, next: Next) -> Response {
        Self::handle(&self, req, next).await
    }
}

impl ContextLayer for A2AContextMiddleware {
    async fn handle(self, req: Request, next: Next) -> Response {
        Self::handle(&self, req, next).await
    }
}

impl ContextLayer for McpContextMiddleware {
    async fn handle(self, req: Request, next: Next) -> Response {
        Self::handle(&self, req, next).await
    }
}

pub trait RouterExt<S>: Sized {
    fn with_rate_limit(
        self,
        limits: &RateLimitState,
        per_second: u64,
        scope: &'static str,
    ) -> Result<Self, LoaderError>;

    fn with_auth<L: ContextLayer>(self, auth: L, policy: AuthzPolicy) -> Self;
}

impl<S> RouterExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_rate_limit(
        self,
        limits: &RateLimitState,
        per_second: u64,
        scope: &'static str,
    ) -> Result<Self, LoaderError> {
        let rate_config = &limits.config;
        if rate_config.disabled {
            return Ok(self);
        }

        // Why: a truncating `as u32` turns any product that is a multiple of 2^32 into
        // a zero burst, which `finish()` reports only by returning `None` —
        // silently leaving the route unlimited. Saturate and clamp so the quota
        // is always representable.
        let burst = per_second.saturating_mul(rate_config.burst_multiplier);
        let burst_u32 = u32::try_from(burst).unwrap_or(u32::MAX).max(1);
        let per_second_clamped = per_second.max(1);

        let rate_limit = tower_governor::governor::GovernorConfigBuilder::default()
            .per_second(per_second_clamped)
            .burst_size(burst_u32)
            .key_extractor(IdentityOrTrustedIpKey::new(Arc::clone(
                &limits.trusted_proxies,
            )))
            .use_headers()
            .finish()
            .ok_or_else(|| LoaderError::InitializationFailed {
                extension: "rate_limit".to_owned(),
                message: format!(
                    "rate limit rejected for {per_second_clamped}/s with burst {burst_u32}"
                ),
            })?;

        let window_secs = u64::try_from(GLOBAL_WINDOW_SECS).unwrap_or(u64::MAX);
        let budget = burst.saturating_mul(window_secs);
        let global = GlobalUserLimit {
            buckets: Arc::clone(&limits.buckets),
            scope,
            budget: i64::try_from(budget).unwrap_or(i64::MAX),
        };

        Ok(self
            .layer(axum::middleware::from_fn_with_state(
                global,
                global_user_rate_limit,
            ))
            .layer(tower_governor::GovernorLayer::new(rate_limit)))
    }

    fn with_auth<L: ContextLayer>(self, auth: L, policy: AuthzPolicy) -> Self {
        self.layer(axum::middleware::from_fn(move |req, next| async move {
            authz_gate(policy, req, next).await
        }))
        .layer(axum::middleware::from_fn(move |req, next| {
            let auth = auth.clone();
            async move { auth.handle(req, next).await }
        }))
    }
}
