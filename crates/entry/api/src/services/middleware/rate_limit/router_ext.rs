//! [`RouterExt`]: the rate limit, bearer window and authentication layers a
//! route group is protected with, applied to the whole router or to its
//! routes alone ([`LayerReach`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use systemprompt_extension::{AuthzPolicy, LoaderError};

use super::bearer::{BearerWindow, bearer_user_rate_limit};
use super::global::global_user_rate_limit;
use super::throttle::{Keying, Throttle, throttle};
use super::{ContextLayer, IdentityOrTrustedIpKey, RateLimitState};
use crate::services::middleware::authz::authz_gate;
use crate::services::middleware::jwt::JwtContextExtractor;

/// Whether a protection layer wraps the whole router (its fallback
/// included) or only its routes. A group merged at the web root shares the
/// tree's fallback, so it is protected on its routes alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerReach {
    Router,
    Routes,
}

pub fn layer_with<S, L>(router: Router<S>, reach: LayerReach, layer: L) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    L: tower::Layer<axum::routing::Route> + Clone + Send + Sync + 'static,
    L::Service: tower::Service<Request> + Clone + Send + Sync + 'static,
    <L::Service as tower::Service<Request>>::Response: axum::response::IntoResponse + 'static,
    <L::Service as tower::Service<Request>>::Error: Into<std::convert::Infallible> + 'static,
    <L::Service as tower::Service<Request>>::Future: Send + 'static,
{
    match reach {
        LayerReach::Router => router.layer(layer),
        LayerReach::Routes => router.route_layer(layer),
    }
}

pub trait RouterExt<S>: Sized {
    fn with_rate_limit(
        self,
        limits: &RateLimitState,
        per_second: u64,
        scope: &'static str,
    ) -> Result<Self, LoaderError> {
        self.with_rate_limit_reaching(limits, per_second, scope, LayerReach::Router)
    }

    fn with_auth<L: ContextLayer>(self, auth: L, policy: AuthzPolicy) -> Self {
        self.with_auth_reaching(auth, policy, LayerReach::Router)
    }

    fn with_bearer_window(
        self,
        limits: &RateLimitState,
        jwt: &Arc<JwtContextExtractor>,
        per_second: u64,
        scope: &'static str,
    ) -> Self {
        self.with_bearer_window_reaching(limits, jwt, per_second, scope, LayerReach::Router)
    }

    fn with_rate_limit_reaching(
        self,
        limits: &RateLimitState,
        per_second: u64,
        scope: &'static str,
        reach: LayerReach,
    ) -> Result<Self, LoaderError>;

    fn with_auth_reaching<L: ContextLayer>(
        self,
        auth: L,
        policy: AuthzPolicy,
        reach: LayerReach,
    ) -> Self;

    fn with_bearer_window_reaching(
        self,
        limits: &RateLimitState,
        jwt: &Arc<JwtContextExtractor>,
        per_second: u64,
        scope: &'static str,
        reach: LayerReach,
    ) -> Self;
}

impl<S> RouterExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_rate_limit_reaching(
        self,
        limits: &RateLimitState,
        per_second: u64,
        scope: &'static str,
        reach: LayerReach,
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

        let router = layer_with(
            self,
            reach,
            axum::middleware::from_fn_with_state(global, global_user_rate_limit),
        );
        Ok(layer_with(
            router,
            reach,
            axum::middleware::from_fn_with_state(local, throttle),
        ))
    }

    fn with_auth_reaching<L: ContextLayer>(
        self,
        auth: L,
        policy: AuthzPolicy,
        reach: LayerReach,
    ) -> Self {
        let router = layer_with(
            self,
            reach,
            axum::middleware::from_fn(move |req, next| async move {
                authz_gate(policy, req, next).await
            }),
        );
        layer_with(
            router,
            reach,
            axum::middleware::from_fn(move |req, next| {
                let auth = auth.clone();
                async move { auth.handle(req, next).await }
            }),
        )
    }

    fn with_bearer_window_reaching(
        self,
        limits: &RateLimitState,
        jwt: &Arc<JwtContextExtractor>,
        per_second: u64,
        scope: &'static str,
        reach: LayerReach,
    ) -> Self {
        if limits.config.disabled {
            return self;
        }
        let window = BearerWindow {
            limit: limits.global_limit(per_second, scope),
            jwt: Arc::clone(jwt),
        };
        layer_with(
            self,
            reach,
            axum::middleware::from_fn_with_state(window, bearer_user_rate_limit),
        )
    }
}

