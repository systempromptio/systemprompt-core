//! The one place a route group gets its protection and its mount point.
//!
//! Two kinds of group are mounted here. The server's own groups are
//! [`KernelRoutes`] declarations, mounted with [`mount`]. The composition's
//! groups arrive as an admission handoff and are mounted with
//! [`mount_admitted`], after the kernel has re-checked every privileged entry
//! in it ([`recheck`](recheck::recheck)) against the constants in
//! `systemprompt_extension::reserved`: owner core id, privileged shapes,
//! reserved prefixes, built-in gateway prefixes and route conflicts.
//!
//! [`protect`] applies a [`RouteProtection`] with the server's own
//! primitives, innermost first: the body limit, the inner protection layers,
//! the rate limit and bearer window (`RouterExt::with_rate_limit` /
//! `with_bearer_window`), the context middleware and authorization gate
//! (`RouterExt::with_auth`), the outer protection layers, and the
//! frame-options stamp. A group merged at the web root is protected on its
//! routes alone (route layers) and any fallback it carries is discarded, so
//! an unmatched path anywhere in the tree never meets that group's gate, and
//! a root group with no route mounts nothing. The tree's one fallback belongs
//! to the kernel's static group: [`protect_root_fallback`] layers that router
//! whole, so its protection (site auth and the public context) covers every
//! page the fallback serves, and [`RootFallbackOwner`] refuses a second claim
//! with [`MountError::SecondRootFallback`]. A nested group is layered whole,
//! its fallback included, which stays inside its base path. [`place`] nests the
//! result under its base path, or merges it at the web root. Only the web root
//! is merged: every other base path, `/track/engagement` included, is nested,
//! so its routes are relative to it and cannot answer outside it.
//!
//! A handler-verified policy (`AuthzPolicy::handler_verified`) pairs only
//! with `ContextKind::HandlerResolved`: the group keeps its body limit, inner
//! layers, rate limit, bearer window, outer layers and frame stamp, and gets
//! no context middleware or gate, because its named verifier admits callers.
//!
//! A protection whose policy admits no caller type and names no verifier,
//! that pairs the public context with a policy excluding anonymous callers,
//! that mixes a handler-verified policy with a context middleware (or the
//! handler-resolved context with a gated policy), or that names a bearer
//! window or protection layer the server has no primitive for, is refused
//! with a typed [`MountError`] at boot. A body limit above
//! [`DEFAULT_BODY_LIMIT_BYTES`] mounts only at the base and with the limit of
//! the kernel's body-limit privileged shape (the agent proxy).
//!
//! The body-limit check sees only the group-level limit a declaration
//! carries. A few routes set their own `DefaultBodyLimit` inside their
//! router, which this module does not see: the document content routes (the
//! configured `files.documents` limit, `routes/documents/mod.rs`), the bridge
//! drive routes (the same limit, `routes/documents/drive.rs`), the upload
//! part `PUT` (the configured part size plus slack, `routes/uploads/mod.rs`)
//! and the managed consumer (1 MiB, `routes/managed/consumer/mod.rs`). They
//! are reviewed at those sites.
//!
//! A group skipped by its role or enable predicate is logged at `info`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admitted;
mod error;
mod guards;
mod kernel_routes;
mod layers;
mod recheck;

pub use admitted::{MountedGatewayRoute, gateway_routes, mount_admitted, protection_of};
pub use error::MountError;
pub use guards::{ClientWritesGuard, ProtectionGuards, SiteAuthGuard};
pub use kernel_routes::{EnablePredicate, KernelRoutes, gateway_protection, path_under};
pub use recheck::recheck;

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::reserved::{PRIVILEGED_SHAPES, WEB_ROOT_BASE_PATH};
use systemprompt_extension::router::{Allowance, BaseRef, BodyLimit};
use systemprompt_extension::{ContextKind, ExtensionContext, RouteGroup, RouteProtection};

use self::layers::{authenticated, inner_layers, outer_layers, rate_limited};
use crate::services::middleware::context::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
use crate::services::middleware::jwt::JwtContextExtractor;
use crate::services::middleware::rate_limit::{LayerReach, RateLimitState};

pub const DEFAULT_BODY_LIMIT_BYTES: usize = 2 * 1024 * 1024;

/// The server's enforcement primitives a [`RouteProtection`] is applied with.
#[derive(Clone)]
pub struct MountLayers {
    pub limits: RateLimitState,
    pub public: PublicContextMiddleware,
    pub user: UserOnlyContextMiddleware,
    pub user_ui: UserOnlyContextMiddleware,
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
    let (router, reach) = if base_path == WEB_ROOT_BASE_PATH {
        if !router.has_routes() {
            return Ok(Router::new());
        }
        (router.reset_fallback(), LayerReach::Routes)
    } else {
        (router, LayerReach::Router)
    };
    apply(router, protection, layers, base_path, reach)
}

/// The web root's one fallback: its owner's router layered whole, so the
/// owner's protection covers every unmatched path the tree serves.
#[derive(Debug)]
pub struct RootFallback {
    router: Router,
    group: RouteGroup,
}

pub fn protect_root_fallback(
    router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    group: RouteGroup,
) -> Result<RootFallback, MountError> {
    check_coherence(protection, WEB_ROOT_BASE_PATH)?;
    check_body_limit(protection, WEB_ROOT_BASE_PATH)?;
    let router = apply(
        router,
        protection,
        layers,
        WEB_ROOT_BASE_PATH,
        LayerReach::Router,
    )?;
    Ok(RootFallback { router, group })
}

/// The group that has placed the web root's fallback in a tree; a second
/// claim is refused.
#[derive(Debug, Default, Clone, Copy)]
pub struct RootFallbackOwner(Option<RouteGroup>);

impl RootFallbackOwner {
    pub fn place(&mut self, tree: Router, fallback: RootFallback) -> Result<Router, MountError> {
        if let Some(held) = self.0 {
            return Err(MountError::SecondRootFallback {
                held,
                claimed: fallback.group,
            });
        }
        self.0 = Some(fallback.group);
        Ok(tree.merge(fallback.router))
    }
}

fn apply(
    router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    base_path: &'static str,
    reach: LayerReach,
) -> Result<Router, MountError> {
    let router = inner_layers(router, protection, layers, base_path, reach)?;
    let router = rate_limited(router, protection, layers, base_path, reach)?;
    let router = authenticated(router, protection, layers, reach);
    outer_layers(router, protection, layers, base_path, reach)
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
        Some(declared)
            if declared > DEFAULT_BODY_LIMIT_BYTES && !admits_body_limit(base_path, declared) =>
        {
            Err(MountError::UnlistedBodyExemption {
                base_path,
                declared,
            })
        },
        _ => Ok(()),
    }
}

fn admits_body_limit(base_path: &str, declared: usize) -> bool {
    PRIVILEGED_SHAPES.iter().any(|entry| {
        entry.allowance == Allowance::BodyLimit
            && entry.shape.limits.body == BodyLimit::Agent
            && matches!(entry.base, BaseRef::Path(base) if base.path() == base_path)
            && declared == systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES
    })
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
    declared: KernelRoutes,
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
