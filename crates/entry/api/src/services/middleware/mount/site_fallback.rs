//! The composition's site fallback as the web root's fallback: public policy
//! and public context only, behind the site-auth layer when a member declares
//! site auth, layered whole, and answering `404` for any path that falls
//! under a reserved entry.
//!
//! Static content answers first ([`RootFallback::behind`]): a `GET` or
//! `HEAD` the static router serves is its answer, and only a response marked
//! [`StaticMiss`] is handed to the site fallback with the original request.
//! Every other method goes to the site fallback directly, since static content
//! is read-only. Every request the site fallback receives carries
//! [`StaticMiss`] as a request extension, so the site-auth gate in front of it
//! never lets a path through for its static-asset extension alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::Method;
use axum::response::Response;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ProtectionLayer, RateBudget, RateLimit, RouteGroup, RouteProtection,
};
use tower::ServiceExt;

use super::{MountError, MountLayers, RootFallback, protect_root_fallback, reserved_guard};
use crate::services::middleware::rate_limit::LayerReach;

pub const SITE_FALLBACK_RATE: RateLimit = RateLimit::new("site_fallback", RateBudget::Content);

pub fn protect_site_fallback(
    fallback: Router,
    layers: &MountLayers,
) -> Result<RootFallback, MountError> {
    let protection = site_fallback_protection(layers.guards.site_auth.is_some());
    let owned = protect_root_fallback(fallback, &protection, layers, RouteGroup::Extensions)?;
    Ok(RootFallback {
        router: reserved_guard::guarded_site(owned.router, LayerReach::Router),
        group: owned.group,
    })
}

/// The marker a static response carries when static content holds nothing
/// for the path; the site fallback answers such a request instead.
#[derive(Debug, Clone, Copy)]
pub struct StaticMiss;

impl RootFallback {
    #[must_use]
    pub fn behind(self, statics: Self) -> Self {
        let site = self.router;
        let statics = statics.router;
        let router = Router::new().fallback_service(tower::service_fn(move |request: Request| {
            let site = site.clone();
            let statics = statics.clone();
            async move { Ok::<_, Infallible>(static_first(statics, site, request).await) }
        }));
        Self {
            router,
            group: self.group,
        }
    }
}

async fn static_first(statics: Router, site: Router, mut request: Request) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        request.extensions_mut().insert(StaticMiss);
        return answer(site, request).await;
    }
    let (parts, body) = request.into_parts();
    let response = answer(statics, Request::from_parts(parts.clone(), Body::empty())).await;
    if response.extensions().get::<StaticMiss>().is_none() {
        return response;
    }
    let mut request = Request::from_parts(parts, body);
    request.extensions_mut().insert(StaticMiss);
    answer(site, request).await
}

async fn answer(router: Router, request: Request) -> Response {
    match router.oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    }
}

#[must_use]
pub fn site_fallback_protection(site_auth: bool) -> RouteProtection {
    let protection = RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
        .with_rate_limit(SITE_FALLBACK_RATE);
    if site_auth {
        protection.with_layer(ProtectionLayer::SiteAuth)
    } else {
        protection
    }
}
