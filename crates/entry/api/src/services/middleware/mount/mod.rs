//! The one place a declared router gets its protection and its mount point.
//!
//! Every route group — core or extension — is described by a
//! [`RouteProtection`] (inside an [`ExtensionRouter`] declaration), and
//! [`protect`] applies it with the server's own primitives, innermost first:
//! the body limit, the inner protection layers, the rate limit and bearer
//! window (`RouterExt::with_rate_limit` / `with_bearer_window`), the context
//! middleware and authorization gate (`RouterExt::with_auth`), the outer
//! protection layers, and the frame-options stamp. [`mount`] adds the role
//! group and enable predicate and nests the result under its base path, or
//! merges it at the web root.
//!
//! A handler-verified policy (`AuthzPolicy::handler_verified`) pairs only
//! with `ContextKind::HandlerResolved`: the group keeps its body limit, inner
//! layers, rate limit, bearer window, outer layers and frame stamp, and gets
//! no context middleware or gate, because its named verifier admits callers.
//!
//! A declaration whose policy admits no caller type and names no verifier,
//! that pairs the public context with a policy excluding anonymous callers,
//! that mixes a handler-verified policy with a context middleware (or the
//! handler-resolved context with a gated policy), or that names a bearer
//! window or protection layer the server has no primitive for, is refused
//! with a typed [`MountError`] at boot.
//!
//! Body limits: a group-level limit above [`DEFAULT_BODY_LIMIT_BYTES`] mounts
//! only when a [`CoreBodyExemption`] names that group's base path and that
//! exact limit; the agent proxy is the only one. [`admit_extension`] refuses
//! any extension limit above the ceiling and any handler-verified extension
//! declaration. This check sees only the group-level limit a declaration
//! carries. A few routes set their own `DefaultBodyLimit` inside their router,
//! which this module does not see: the document content routes (the
//! configured `files.documents` limit, `routes/documents/mod.rs`), the bridge
//! drive routes (the same limit, `routes/documents/drive.rs`), the upload
//! part `PUT` (the configured part size plus slack, `routes/uploads/mod.rs`)
//! and the managed consumer (1 MiB, `routes/managed/consumer/mod.rs`). They
//! are reviewed at those sites.
//!
//! A declaration skipped by its role group or enable predicate is logged at
//! `info`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod exemption;
mod guards;

pub use error::MountError;
pub use exemption::CoreBodyExemption;
pub use guards::{ClientWritesGuard, ProtectionGuards, SiteAuthGuard};

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use systemprompt_extension::registry::WEB_ROOT_BASE_PATH;
use systemprompt_extension::{
    ContextKind, ExtensionContext, ExtensionRouter, ProtectionLayer, RouteGroup, RouteProtection,
};

use crate::services::middleware::context::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
use crate::services::middleware::jwt::JwtContextExtractor;
use crate::services::middleware::rate_limit::{RateLimitState, RouterExt};

pub const DEFAULT_BODY_LIMIT_BYTES: usize = 2 * 1024 * 1024;

/// The server's enforcement primitives a [`RouteProtection`] is applied with.
#[derive(Clone)]
pub struct MountLayers {
    pub limits: RateLimitState,
    pub public: PublicContextMiddleware,
    pub user: UserOnlyContextMiddleware,
    pub a2a: A2AContextMiddleware,
    pub mcp: McpContextMiddleware,
    pub bearer: Option<Arc<JwtContextExtractor>>,
    pub guards: ProtectionGuards,
}

impl std::fmt::Debug for MountLayers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountLayers")
            .field("limits", &self.limits)
            .field("guards", &self.guards)
            .field("bearer_window_available", &self.bearer.is_some())
            .finish_non_exhaustive()
    }
}

pub fn protect(
    router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    base_path: &'static str,
) -> Result<Router, MountError> {
    check_coherence(protection, base_path)?;
    check_body_limit(protection, base_path)?;
    let router = inner_layers(router, protection, layers, base_path)?;
    let router = rate_limited(router, protection, layers, base_path)?;
    let router = authenticated(router, protection, layers);
    outer_layers(router, protection, layers, base_path)
}

fn check_coherence(
    protection: &RouteProtection,
    base_path: &'static str,
) -> Result<(), MountError> {
    let handler_verified = protection.policy.handler_verifier().is_some();
    if handler_verified != (protection.context == ContextKind::HandlerResolved) {
        return Err(MountError::IncoherentHandlerVerification { base_path });
    }
    if !handler_verified && protection.policy.allowed().is_empty() {
        return Err(MountError::NoAdmittedCaller { base_path });
    }
    if protection.context == ContextKind::Public && protection.requires_authentication() {
        return Err(MountError::IncoherentContext { base_path });
    }
    Ok(())
}

fn check_body_limit(
    protection: &RouteProtection,
    base_path: &'static str,
) -> Result<(), MountError> {
    match protection.body_limit {
        Some(declared) if !CoreBodyExemption::admits(base_path, declared) => {
            Err(MountError::UnlistedBodyExemption {
                base_path,
                declared,
            })
        },
        _ => Ok(()),
    }
}

fn inner_layers(
    mut router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    base_path: &'static str,
) -> Result<Router, MountError> {
    if let Some(bytes) = protection.body_limit {
        router = router.layer(DefaultBodyLimit::max(bytes));
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
        router = router.layer(axum::middleware::from_fn(move |request, next| {
            guard.clone().refuse(request, next)
        }));
    }
    Ok(router)
}

fn rate_limited(
    mut router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    base_path: &'static str,
) -> Result<Router, MountError> {
    let Some(rate_limit) = protection.rate_limit else {
        return Ok(router);
    };
    let per_second = rate_limit.budget.per_second(layers.limits.config());
    router = router
        .with_rate_limit(&layers.limits, per_second, rate_limit.scope)
        .map_err(|source| MountError::RateLimit {
            base_path,
            source: Box::new(source),
        })?;
    if rate_limit.bearer_window {
        let jwt = layers
            .bearer
            .as_ref()
            .ok_or(MountError::BearerWindowUnavailable { base_path })?;
        router = router.with_bearer_window(&layers.limits, jwt, per_second, rate_limit.scope);
    }
    Ok(router)
}

fn authenticated(router: Router, protection: &RouteProtection, layers: &MountLayers) -> Router {
    let policy = protection.policy;
    match protection.context {
        ContextKind::Public => router.with_auth(layers.public, policy),
        ContextKind::User => router.with_auth(layers.user.clone(), policy),
        ContextKind::A2a => router.with_auth(layers.a2a.clone(), policy),
        ContextKind::Mcp => router.with_auth(layers.mcp.clone(), policy),
        ContextKind::HandlerResolved => router,
    }
}

fn outer_layers(
    mut router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    base_path: &'static str,
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
        router = router.layer(axum::middleware::from_fn(move |request, next| {
            guard.clone().gate(request, next)
        }));
    }

    if let Some(frame_options) = protection.frame_options {
        router = router.layer(axum::middleware::from_fn(move |request, next| {
            systemprompt_extension::stamp_frame_options(frame_options, request, next)
        }));
    }

    Ok(router)
}

pub fn place(tree: Router, base_path: &'static str, router: Router) -> Router {
    if base_path == WEB_ROOT_BASE_PATH {
        tree.merge(router)
    } else {
        tree.nest(base_path, router)
    }
}

pub fn mount(
    tree: Router,
    declared: ExtensionRouter,
    layers: &MountLayers,
    ctx: &dyn ExtensionContext,
    serves: &dyn Fn(RouteGroup) -> bool,
) -> Result<Router, MountError> {
    if !serves(declared.group) {
        tracing::info!(
            base_path = declared.base_path,
            group = ?declared.group,
            reason = "role_does_not_serve_group",
            "Route declaration not mounted"
        );
        return Ok(tree);
    }
    if !declared.is_enabled(ctx) {
        tracing::info!(
            base_path = declared.base_path,
            group = ?declared.group,
            reason = "enable_predicate_false",
            "Route declaration not mounted"
        );
        return Ok(tree);
    }
    let base_path = declared.base_path;
    let protected = protect(declared.router, &declared.protection, layers, base_path)?;
    Ok(place(tree, base_path, protected))
}

pub const fn admit_extension(declared: &ExtensionRouter) -> Result<(), MountError> {
    if declared.protection.policy.handler_verifier().is_some()
        || matches!(declared.protection.context, ContextKind::HandlerResolved)
    {
        return Err(MountError::ExtensionHandlerVerified {
            base_path: declared.base_path,
        });
    }
    match declared.protection.body_limit {
        Some(declared_bytes) if declared_bytes > DEFAULT_BODY_LIMIT_BYTES => {
            Err(MountError::ExtensionBodyLimitTooLarge {
                base_path: declared.base_path,
                declared: declared_bytes,
                ceiling: DEFAULT_BODY_LIMIT_BYTES,
            })
        },
        _ => Ok(()),
    }
}
