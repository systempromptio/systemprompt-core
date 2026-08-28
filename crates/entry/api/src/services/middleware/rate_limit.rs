//! Router extension traits for rate limiting and authenticated route groups.
//!
//! `RouterExt::with_auth` attaches authentication and authorization in one
//! call: it requires an `AuthzPolicy`, so a route group cannot be mounted
//! authenticated-but-unauthorized — omitting the policy is a compile error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::middleware::authz::{AuthzPolicy, authz_gate};
use crate::services::middleware::client_addr::resolve_client_ip;
use crate::services::middleware::context::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
use axum::Router;
use axum::extract::{ConnectInfo, Request};
use axum::middleware::Next;
use axum::response::Response;
use ipnet::IpNet;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_extension::LoaderError;
use systemprompt_models::auth::UserType;
use systemprompt_models::{Config, RequestContext};

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
    fn with_rate_limit(self, config: &Config, per_second: u64) -> Result<Self, LoaderError>;

    fn with_auth<L: ContextLayer>(self, auth: L, policy: AuthzPolicy) -> Self;
}

impl<S> RouterExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_rate_limit(self, config: &Config, per_second: u64) -> Result<Self, LoaderError> {
        let rate_config = &config.rate_limits;
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
            .key_extractor(tower_governor::key_extractor::SmartIpKeyExtractor)
            .use_headers()
            .finish()
            .ok_or_else(|| LoaderError::InitializationFailed {
                extension: "rate_limit".to_owned(),
                message: format!(
                    "rate limit rejected for {per_second_clamped}/s with burst {burst_u32}"
                ),
            })?;

        Ok(self.layer(tower_governor::GovernorLayer::new(rate_limit)))
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
