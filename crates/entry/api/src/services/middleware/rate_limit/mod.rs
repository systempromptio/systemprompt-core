//! Router extension traits for rate limiting and authenticated route groups.
//!
//! `RouterExt::with_auth` attaches authentication and authorization in one
//! call: it requires an `AuthzPolicy`, so a route group cannot be mounted
//! authenticated-but-unauthorized — omitting the policy is a compile error.
//!
//! `RouterExt::with_rate_limit` mounts two throttles: the in-process limiter
//! keyed by verified identity or trusted client IP, which smooths bursts per
//! replica, and a database-backed window keyed by verified identity only,
//! which bounds a caller's budget across every replica of the deployment.
//!
//! [`with_pre_auth_ip_limit`] mounts a third, cheaper throttle on the whole
//! server, outside every authenticating layer: an in-process limiter keyed by
//! resolved client address only. `with_auth` decodes the bearer and checks
//! its revocation before the identity-keyed throttles see the request, so
//! without it a flood of forged or expired credentials would cost a token
//! decode and a database lookup each, unthrottled.
//!
//! A route that verifies its caller inside the handler from a credential in
//! the request (the internal links route and its MCP hop token) takes a
//! [`SubjectThrottle`] from [`RateLimitState::subject_throttle`] and charges
//! it with the verified subject, so callers sharing one address (every
//! managed MCP server calls from loopback) never share one budget.
//!
//! On routers that carry `with_auth` the database-backed window reads the
//! identity from the [`RequestContext`](systemprompt_models::RequestContext)
//! in the request's extensions. A router that authenticates inside its
//! handlers never puts one there, so it adds `RouterExt::with_bearer_window`,
//! which charges the same window by the signature-verified bearer token;
//! without it the per-replica limiter would be the only limit, and the
//! effective budget would multiply with the replica count. A `worker` mounts
//! its MCP and agent routes with [`RateLimitState::unmetered`]: only `api`
//! replicas that already charged the caller forward to it, and charging the
//! same window twice would halve every forwarded caller's budget.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bearer;
mod global;
mod key;
pub mod keyed;
mod throttle;

use crate::services::middleware::authz::authz_gate;
use crate::services::middleware::context::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
use crate::services::middleware::jwt::JwtContextExtractor;
use axum::Router;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use ipnet::IpNet;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_extension::{AuthzPolicy, LoaderError};
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserRateLimitBucketRepository;

use self::bearer::{BearerWindow, bearer_user_rate_limit};
use self::global::{GLOBAL_WINDOW_SECS, GlobalUserLimit, global_user_rate_limit};

pub use self::bearer::bearer_jwt;
pub use self::key::{IdentityOrTrustedIpKey, TrustedIpKey, client_bucket};
pub use self::keyed::KeyedLimiter;
use self::throttle::{Keying, Throttle, throttle};
pub use self::throttle::{SubjectThrottle, Throttled};

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

    pub fn from_context(ctx: &AppContext) -> Self {
        let buckets = crate::repository::user_rate_limit_buckets(ctx.db_pool());
        Self::new(ctx.config(), buckets)
    }

    #[must_use]
    pub const fn unmetered(mut self) -> Self {
        self.config.disabled = true;
        self
    }

    #[must_use]
    pub fn subject_throttle(&self, per_second: u64) -> Option<Arc<SubjectThrottle>> {
        if self.config.disabled {
            return None;
        }
        let (replenish, burst) = self.limiter_shape(per_second);
        Some(SubjectThrottle::new(replenish, burst))
    }

    fn limiter_shape(&self, per_second: u64) -> (Duration, u32) {
        let burst_u32 = u32::try_from(self.burst(per_second))
            .unwrap_or(u32::MAX)
            .max(1);
        let per_second_clamped = per_second.max(1);
        let replenish = Duration::from_secs(1)
            .checked_div(u32::try_from(per_second_clamped).unwrap_or(u32::MAX))
            .filter(|d| !d.is_zero())
            .unwrap_or(Duration::from_nanos(1));
        (replenish, burst_u32)
    }

    const fn burst(&self, per_second: u64) -> u64 {
        per_second.saturating_mul(self.config.burst_multiplier)
    }

    fn global_limit(&self, per_second: u64, scope: &'static str) -> GlobalUserLimit {
        let window_secs = u64::try_from(GLOBAL_WINDOW_SECS).unwrap_or(u64::MAX);
        let budget = self.burst(per_second).saturating_mul(window_secs);
        GlobalUserLimit {
            buckets: Arc::clone(&self.buckets),
            scope,
            budget: i64::try_from(budget).unwrap_or(i64::MAX),
        }
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

    fn with_bearer_window(
        self,
        limits: &RateLimitState,
        jwt: &Arc<JwtContextExtractor>,
        per_second: u64,
        scope: &'static str,
    ) -> Self;
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

        let (replenish, burst_u32) = limits.limiter_shape(per_second);
        let local = Throttle::new(
            replenish,
            burst_u32,
            Keying::IdentityOrIp(IdentityOrTrustedIpKey::new(Arc::clone(
                &limits.trusted_proxies,
            ))),
        );

        let global = limits.global_limit(per_second, scope);

        Ok(self
            .layer(axum::middleware::from_fn_with_state(
                global,
                global_user_rate_limit,
            ))
            .layer(axum::middleware::from_fn_with_state(local, throttle)))
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

    fn with_bearer_window(
        self,
        limits: &RateLimitState,
        jwt: &Arc<JwtContextExtractor>,
        per_second: u64,
        scope: &'static str,
    ) -> Self {
        if limits.config.disabled {
            return self;
        }
        let window = BearerWindow {
            limit: limits.global_limit(per_second, scope),
            jwt: Arc::clone(jwt),
        };
        self.layer(axum::middleware::from_fn_with_state(
            window,
            bearer_user_rate_limit,
        ))
    }
}

pub fn with_pre_auth_ip_limit<S>(router: Router<S>, limits: &RateLimitState) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if limits.config.disabled {
        return router;
    }
    let (replenish, burst) = limits.limiter_shape(limits.config.pre_auth_ip_per_second);
    let local = Throttle::new(
        replenish,
        burst,
        Keying::Ip(TrustedIpKey::new(Arc::clone(&limits.trusted_proxies))),
    );
    router.layer(axum::middleware::from_fn_with_state(local, throttle))
}
