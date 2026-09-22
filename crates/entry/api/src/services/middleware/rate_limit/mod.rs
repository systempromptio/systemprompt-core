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
//! The database-backed window applies only to routers that carry `with_auth`:
//! the identity comes from a
//! [`RequestContext`](systemprompt_models::RequestContext) in the request's
//! extensions, and a router that authenticates inside its handlers never puts
//! one there. On those routers every request is anonymous to that layer and
//! passes straight through, so the per-address governor is the only limit in
//! force.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod global;
mod key;

use crate::services::middleware::authz::{AuthzPolicy, authz_gate};
use crate::services::middleware::context::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
use axum::Router;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use ipnet::IpNet;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_extension::LoaderError;
use systemprompt_models::Config;
use systemprompt_models::profile::RateLimitsConfig;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserRateLimitBucketRepository;

use self::global::{GLOBAL_WINDOW_SECS, GlobalUserLimit, global_user_rate_limit};
pub use self::key::IdentityOrTrustedIpKey;

#[derive(Clone, Debug)]
pub struct RateLimitState {
    config: RateLimitsConfig,
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

        let burst = per_second.saturating_mul(rate_config.burst_multiplier);
        let burst_u32 = u32::try_from(burst).unwrap_or(u32::MAX).max(1);
        let per_second_clamped = per_second.max(1);

        // Why: `GovernorConfigBuilder::per_second(n)` replenishes one element
        // every n seconds, the inverse of a rate; `period` takes 1/per_second
        // directly.
        let replenish = Duration::from_secs(1)
            .checked_div(u32::try_from(per_second_clamped).unwrap_or(u32::MAX))
            .filter(|d| !d.is_zero())
            .unwrap_or(Duration::from_nanos(1));
        let rate_limit = tower_governor::governor::GovernorConfigBuilder::default()
            .period(replenish)
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
