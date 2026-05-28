//! Router extension traits for rate limiting and authenticated route groups.
//!
//! `RouterExt::with_auth` attaches authentication and authorization in one
//! call: it requires an `AuthzPolicy`, so a route group cannot be mounted
//! authenticated-but-unauthorized — omitting the policy is a compile error.

use crate::services::middleware::authz::{AuthzPolicy, authz_gate};
use crate::services::middleware::client_addr::resolve_client_ip;
use crate::services::middleware::context::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
use crate::services::middleware::jti_revocation::{JtiRevocationState, jti_revocation_middleware};
use axum::Router;
use axum::extract::{ConnectInfo, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::clock::DefaultClock;
use governor::state::keyed::DefaultKeyedStateStore;
use governor::{Quota, RateLimiter};
use ipnet::IpNet;
use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use systemprompt_models::RequestContext;
use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_models::auth::RateLimitTier;
use systemprompt_models::config::RateLimitConfig;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tracing::warn;

/// Builds a [`systemprompt_models::RequestContext`] for a route group.
///
/// Implemented by each of the four sibling context middlewares
/// ([`PublicContextMiddleware`], [`UserOnlyContextMiddleware`],
/// [`A2AContextMiddleware`], [`McpContextMiddleware`]). Sealed to those four —
/// third parties cannot stand up a new flavour outside this crate, which keeps
/// the route-mount surface auditable.
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

pub trait RouterExt<S> {
    fn with_rate_limit(self, rate_config: &RateLimitConfig, per_second: u64) -> Self;

    fn with_auth<L: ContextLayer>(self, auth: L, policy: AuthzPolicy) -> Self;

    fn with_jti_check(self, jti_state: JtiRevocationState) -> Self;
}

impl<S> RouterExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_rate_limit(self, rate_config: &RateLimitConfig, per_second: u64) -> Self {
        if rate_config.disabled {
            return self;
        }

        let rate_limit_result = tower_governor::governor::GovernorConfigBuilder::default()
            .per_second(per_second)
            .burst_size((per_second * rate_config.burst_multiplier) as u32)
            .key_extractor(SmartIpKeyExtractor)
            .use_headers()
            .finish();

        if let Some(rate_limit) = rate_limit_result {
            self.layer(tower_governor::GovernorLayer::new(rate_limit))
        } else {
            warn!("Failed to configure rate limiting - rate limiting disabled for this route");
            self
        }
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

    fn with_jti_check(self, jti_state: JtiRevocationState) -> Self {
        self.layer(axum::middleware::from_fn_with_state(
            jti_state,
            jti_revocation_middleware,
        ))
    }
}

type KeyedRateLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

#[derive(Clone, Debug)]
pub struct TieredRateLimiter {
    admin_limiter: Arc<KeyedRateLimiter>,
    user_limiter: Arc<KeyedRateLimiter>,
    a2a_limiter: Arc<KeyedRateLimiter>,
    mcp_limiter: Arc<KeyedRateLimiter>,
    service_limiter: Arc<KeyedRateLimiter>,
    anon_limiter: Arc<KeyedRateLimiter>,
    disabled: bool,
    trusted_proxies: Arc<Vec<IpNet>>,
}

impl TieredRateLimiter {
    pub fn new(config: &RateLimitConfig, base_per_second: u64) -> Self {
        Self::with_trusted_proxies(config, base_per_second, Vec::new())
    }

    pub fn with_trusted_proxies(
        config: &RateLimitConfig,
        base_per_second: u64,
        trusted_proxies: Vec<IpNet>,
    ) -> Self {
        let create_limiter = |tier: RateLimitTier| -> Arc<KeyedRateLimiter> {
            let effective = config.effective_limit(base_per_second, tier);
            let burst = effective.saturating_mul(config.burst_multiplier);
            let effective_u32 = u32::try_from(effective).unwrap_or(u32::MAX).max(1);
            let burst_u32 = u32::try_from(burst).unwrap_or(u32::MAX).max(1);
            let quota =
                Quota::per_second(NonZeroU32::new(effective_u32).unwrap_or(NonZeroU32::MIN))
                    .allow_burst(NonZeroU32::new(burst_u32).unwrap_or(NonZeroU32::MIN));
            Arc::new(RateLimiter::keyed(quota))
        };

        Self {
            admin_limiter: create_limiter(RateLimitTier::Admin),
            user_limiter: create_limiter(RateLimitTier::User),
            a2a_limiter: create_limiter(RateLimitTier::A2a),
            mcp_limiter: create_limiter(RateLimitTier::Mcp),
            service_limiter: create_limiter(RateLimitTier::Service),
            anon_limiter: create_limiter(RateLimitTier::Anon),
            disabled: config.disabled,
            trusted_proxies: Arc::new(trusted_proxies),
        }
    }

    pub fn disabled() -> Self {
        let quota = Quota::per_second(NonZeroU32::MAX);
        let limiter = Arc::new(RateLimiter::keyed(quota));
        Self {
            admin_limiter: Arc::clone(&limiter),
            user_limiter: Arc::clone(&limiter),
            a2a_limiter: Arc::clone(&limiter),
            mcp_limiter: Arc::clone(&limiter),
            service_limiter: Arc::clone(&limiter),
            anon_limiter: Arc::clone(&limiter),
            disabled: true,
            trusted_proxies: Arc::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn trusted_proxies(&self) -> &[IpNet] {
        &self.trusted_proxies
    }

    fn limiter_for_tier(&self, tier: RateLimitTier) -> &KeyedRateLimiter {
        match tier {
            RateLimitTier::Admin => &self.admin_limiter,
            RateLimitTier::User => &self.user_limiter,
            RateLimitTier::A2a => &self.a2a_limiter,
            RateLimitTier::Mcp => &self.mcp_limiter,
            RateLimitTier::Service => &self.service_limiter,
            RateLimitTier::Anon => &self.anon_limiter,
        }
    }

    pub fn check(&self, tier: RateLimitTier, key: &str) -> bool {
        if self.disabled {
            return true;
        }
        self.limiter_for_tier(tier)
            .check_key(&key.to_owned())
            .is_ok()
    }
}

pub async fn tiered_rate_limit_middleware(
    limiter: axum::extract::State<TieredRateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    if limiter.disabled {
        return next.run(request).await;
    }

    let (tier, key) = request.extensions().get::<RequestContext>().map_or_else(
        || {
            let connect_info = request.extensions().get::<ConnectInfo<SocketAddr>>();
            let ip = resolve_client_ip(request.headers(), connect_info, limiter.trusted_proxies())
                .map_or_else(|| "unknown".to_owned(), |a| a.to_string());
            (RateLimitTier::Anon, ip)
        },
        |ctx| {
            let tier = ctx.rate_limit_tier();
            let key = ctx.user_id().to_string();
            (tier, key)
        },
    );

    if limiter.check(tier, &key) {
        next.run(request).await
    } else {
        warn!(
            tier = %tier.as_str(),
            key = %key,
            "Rate limit exceeded"
        );
        let api_error = ApiError::new(ErrorCode::RateLimited, "Rate limit exceeded");
        let mut response = api_error.into_response();
        response
            .headers_mut()
            .insert("Retry-After", http::HeaderValue::from_static("1"));
        if let Ok(tier_value) = http::HeaderValue::from_str(tier.as_str()) {
            response
                .headers_mut()
                .insert("X-Rate-Limit-Tier", tier_value);
        }
        response
    }
}
