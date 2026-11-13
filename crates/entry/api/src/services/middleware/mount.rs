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
//! A declaration whose policy admits no caller type, or that names a bearer
//! window or protection layer the server has no primitive for, is refused
//! with a typed [`MountError`] at boot; nothing is mounted with less
//! protection than it declares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_extension::registry::WEB_ROOT_BASE_PATH;
use systemprompt_extension::{
    ContextKind, ExtensionContext, ExtensionRouter, LoaderError, ProtectionLayer, RouteGroup,
    RouteProtection, SiteAuthConfig,
};
use systemprompt_models::ApiError;
use systemprompt_oauth_issuance::IDENTITY_PLANE;
use systemprompt_runtime::AppContext;
use systemprompt_security::keys::SigningKeys;

use crate::error::ApiHttpError;
use crate::services::middleware::context::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
use crate::services::middleware::jwt::JwtContextExtractor;
use crate::services::middleware::rate_limit::{RateLimitState, RouterExt};
use crate::services::middleware::site_auth::{SiteAuthSession, site_auth_gate};
use crate::services::session_cookie::SessionCookie;

const CLIENTS_PATH: &str = "/clients";

#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("route group at '{base_path}' declares a policy that admits no caller type")]
    NoAdmittedCaller { base_path: &'static str },

    #[error(
        "route group at '{base_path}' declares a bearer window but the server has no bearer \
         extractor"
    )]
    BearerWindowUnavailable { base_path: &'static str },

    #[error(
        "route group at '{base_path}' declares the {layer:?} protection layer but the server has \
         no guard for it"
    )]
    GuardUnavailable {
        base_path: &'static str,
        layer: ProtectionLayer,
    },

    #[error("route group at '{base_path}' could not build its rate limit")]
    RateLimit {
        base_path: &'static str,
        #[source]
        source: LoaderError,
    },
}

/// The site-auth gate an extension's [`SiteAuthConfig`] installs over the
/// static pages.
#[derive(Clone)]
pub struct SiteAuthGuard {
    pub config: SiteAuthConfig,
    pub issuer: Arc<str>,
    pub keys: SigningKeys,
    pub cookie: SessionCookie,
}

impl SiteAuthGuard {
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Option<Self> {
        let config = ctx
            .extension_registry()
            .extensions()
            .iter()
            .find_map(|ext| ext.site_auth())?;
        Some(Self {
            config,
            issuer: Arc::from(ctx.config().jwt_issuer.as_str()),
            keys: ctx.signing_keys().clone(),
            cookie: SessionCookie::from_config(ctx.config()),
        })
    }

    async fn gate(self, request: Request, next: Next) -> Response {
        site_auth_gate(
            request,
            next,
            self.config,
            SiteAuthSession {
                issuer: &self.issuer,
                keys: &self.keys,
                cookie: self.cookie,
            },
        )
        .await
    }
}

/// Refuses the client-management writes (`POST`/`PUT`/`DELETE` under
/// `/clients`) while the identity config plane is file-managed.
#[derive(Clone)]
pub struct ClientWritesGuard {
    ctx: AppContext,
}

impl ClientWritesGuard {
    #[must_use]
    pub const fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }

    async fn refuse(self, request: Request, next: Next) -> Response {
        if request.uri().path().starts_with(CLIENTS_PATH) && !request.method().is_safe() {
            let refused = self
                .ctx
                .config_planes()
                .and_then(|planes| planes.writer(IDENTITY_PLANE).map(|_| ()));
            if let Err(error) = refused {
                return ApiHttpError::from(ApiError::from(error)).into_response();
            }
        }
        next.run(request).await
    }
}

/// The primitives each [`ProtectionLayer`] is applied with.
#[derive(Clone, Default)]
pub struct ProtectionGuards {
    pub site_auth: Option<SiteAuthGuard>,
    pub client_writes: Option<ClientWritesGuard>,
}

impl ProtectionGuards {
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        Self {
            site_auth: SiteAuthGuard::from_context(ctx),
            client_writes: Some(ClientWritesGuard::new(ctx.clone())),
        }
    }
}

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

pub fn protect(
    router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    base_path: &'static str,
) -> Result<Router, MountError> {
    if protection.policy.allowed().is_empty() {
        return Err(MountError::NoAdmittedCaller { base_path });
    }

    let mut router = router;

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

    if let Some(rate_limit) = protection.rate_limit {
        let per_second = rate_limit.budget.per_second(layers.limits.config());
        router = router
            .with_rate_limit(&layers.limits, per_second, rate_limit.scope)
            .map_err(|source| MountError::RateLimit { base_path, source })?;
        if rate_limit.bearer_window {
            let jwt = layers
                .bearer
                .as_ref()
                .ok_or(MountError::BearerWindowUnavailable { base_path })?;
            router = router.with_bearer_window(&layers.limits, jwt, per_second, rate_limit.scope);
        }
    }

    let policy = protection.policy;
    router = match protection.context {
        ContextKind::Public => router.with_auth(layers.public, policy),
        ContextKind::User => router.with_auth(layers.user.clone(), policy),
        ContextKind::A2a => router.with_auth(layers.a2a.clone(), policy),
        ContextKind::Mcp => router.with_auth(layers.mcp.clone(), policy),
    };

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

#[must_use]
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
    if !serves(declared.group) || !declared.is_enabled(ctx) {
        return Ok(tree);
    }
    let base_path = declared.base_path;
    let protected = protect(declared.router, &declared.protection, layers, base_path)?;
    Ok(place(tree, base_path, protected))
}
