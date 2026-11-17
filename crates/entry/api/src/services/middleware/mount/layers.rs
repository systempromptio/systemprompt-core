//! The protection layers [`super::protect`] applies, innermost first: the
//! body limit and inner protection layers, the rate limit and bearer window,
//! the context middleware and authorization gate, and the outer protection
//! layers with the frame-options stamp.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::extract::DefaultBodyLimit;
use systemprompt_extension::{ContextKind, ProtectionLayer, RouteProtection};

use super::{MountError, MountLayers};
use crate::services::middleware::rate_limit::{LayerReach, RouterExt, layer_with};

pub(super) fn inner_layers(
    mut router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    base_path: &'static str,
    reach: LayerReach,
) -> Result<Router, MountError> {
    if let Some(bytes) = protection.body_limit {
        router = layer_with(router, reach, DefaultBodyLimit::max(bytes));
    }
    if protection
        .layers
        .contains(&ProtectionLayer::RefuseFileManagedClientWrites)
    {
        let guard = layers
            .guards
            .client_writes
            .clone()
            .ok_or(MountError::GuardUnavailable {
                base_path,
                layer: ProtectionLayer::RefuseFileManagedClientWrites,
            })?;
        router = layer_with(
            router,
            reach,
            axum::middleware::from_fn(move |request, next| guard.clone().refuse(request, next)),
        );
    }
    Ok(router)
}

pub(super) fn rate_limited(
    mut router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    base_path: &'static str,
    reach: LayerReach,
) -> Result<Router, MountError> {
    let Some(rate_limit) = protection.rate_limit else {
        return Ok(router);
    };
    let per_second = rate_limit.budget.per_second(layers.limits.config());
    router = router
        .with_rate_limit_reaching(&layers.limits, per_second, rate_limit.scope, reach)
        .map_err(|source| MountError::RateLimit {
            base_path,
            source: Box::new(source),
        })?;
    if rate_limit.bearer_window {
        let jwt = layers
            .bearer
            .as_ref()
            .ok_or(MountError::BearerWindowUnavailable { base_path })?;
        router = router.with_bearer_window_reaching(
            &layers.limits,
            jwt,
            per_second,
            rate_limit.scope,
            reach,
        );
    }
    Ok(router)
}

pub(super) fn authenticated(
    router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    reach: LayerReach,
) -> Router {
    let policy = protection.policy;
    match protection.context {
        ContextKind::Public => router.with_auth_reaching(layers.public, policy, reach),
        ContextKind::User => router.with_auth_reaching(layers.user.clone(), policy, reach),
        ContextKind::A2a => router.with_auth_reaching(layers.a2a.clone(), policy, reach),
        ContextKind::Mcp => router.with_auth_reaching(layers.mcp.clone(), policy, reach),
        ContextKind::HandlerResolved => router,
    }
}

pub(super) fn outer_layers(
    mut router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    base_path: &'static str,
    reach: LayerReach,
) -> Result<Router, MountError> {
    if protection.layers.contains(&ProtectionLayer::SiteAuth) {
        let guard = layers
            .guards
            .site_auth
            .clone()
            .ok_or(MountError::GuardUnavailable {
                base_path,
                layer: ProtectionLayer::SiteAuth,
            })?;
        router = layer_with(
            router,
            reach,
            axum::middleware::from_fn(move |request, next| guard.clone().gate(request, next)),
        );
    }

    if let Some(frame_options) = protection.frame_options {
        router = layer_with(
            router,
            reach,
            axum::middleware::from_fn(move |request, next| {
                systemprompt_extension::stamp_frame_options(frame_options, request, next)
            }),
        );
    }

    Ok(router)
}
