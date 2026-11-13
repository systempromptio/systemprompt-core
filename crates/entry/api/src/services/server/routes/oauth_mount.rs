//! OAuth and SSO route mounting.
//!
//! Builds the `OAuthState` (with `WebAuthn` when configured) and nests the
//! public and authenticated OAuth routers and the SSO router under their
//! `ApiPaths` bases. The authenticated router carries the
//! `RefuseFileManagedClientWrites` protection layer: while the `identity`
//! config plane is file-managed, client-management writes (`POST`/`PUT`/
//! `DELETE` under `/clients`) are refused with 409, because the declaration
//! under `services/identity/` is the source of truth.
//!
//! The MCP server registry that maps an authorization `resource` to its MCP
//! scopes is read from the extension state, where the `mcp` extension
//! provides it as `Arc<dyn McpRegistryProvider>`. When no extension provides
//! it, the state carries no registry and an authorization request that names
//! a `resource` is refused with `invalid_target`; it never falls back to the
//! client's registered scopes. A request without a `resource` is unaffected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use std::sync::Arc;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionContext as _, LoaderError, ProtectionLayer, RateBudget,
    RateLimit, RouteProtection,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::services::WebAuthnService;
use systemprompt_oauth::{OAuthSigning, OAuthState};
use systemprompt_oauth_issuance::JwksCache;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as AppContextTrait, McpRegistryProvider};

use super::protocol::MountCtx;
use super::{RouteMountError, require_provider};
use crate::services::middleware::place;

fn create_oauth_state(ctx: &AppContext) -> Result<OAuthState, LoaderError> {
    let analytics = require_provider("oauth", ctx.analytics_provider())?;
    let users = require_provider("oauth", ctx.user_provider())?;
    let sessions = require_provider("oauth", ctx.session_provider())?;
    let mcp_registry = ctx
        .extension_state()
        .get::<Arc<dyn McpRegistryProvider>>()
        .cloned();
    let oauth_repository = ctx.oauth_repositories().oauth.clone();
    let webauthn = WebAuthnService::new(ctx.config(), oauth_repository.clone(), Arc::clone(&users));
    let state = OAuthState::new(
        oauth_repository,
        analytics,
        sessions,
        users,
        OAuthSigning {
            keys: ctx.signing_keys().clone(),
            authority: ctx.token_authority(),
        },
    );
    let state = match mcp_registry {
        Ok(registry) => state.with_mcp_registry(registry),
        Err(absent) => {
            tracing::info!(
                missing = %absent,
                "No MCP registry; authorization requests naming a resource are refused"
            );
            state
        },
    };
    match webauthn {
        Ok(service) => Ok(state.with_webauthn(Arc::new(service))),
        Err(e) => {
            tracing::warn!(error = %e, "WebAuthn is not configured; passkey routes will refuse");
            Ok(state)
        },
    }
}

pub(super) fn mount_oauth(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let oauth_state = create_oauth_state(ctx)?;
    let sso_state = crate::routes::sso::SsoRouteState::build(ctx, &oauth_state).map_err(|e| {
        LoaderError::InitializationStepFailed {
            extension: ExtensionId::new("sso"),
            source: Box::new(e),
        }
    })?;
    let sso = mount.protect(
        crate::routes::sso::router(sso_state),
        &RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
            .with_rate_limit(RateLimit::new("sso", RateBudget::OauthPublic)),
        crate::routes::sso::SSO_BASE,
    )?;
    let public = mount.protect(
        crate::routes::oauth::public_router(Arc::new(JwksCache::new()))
            .with_state(oauth_state.clone()),
        &RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
            .with_rate_limit(RateLimit::new("oauth_public", RateBudget::OauthPublic)),
        ApiPaths::OAUTH_BASE,
    )?;
    let authenticated = mount.protect(
        crate::routes::oauth::authenticated_router().with_state(oauth_state),
        &RouteProtection::new(AuthzPolicy::user(), ContextKind::User)
            .with_layer(ProtectionLayer::RefuseFileManagedClientWrites)
            .with_rate_limit(RateLimit::new("oauth_auth", RateBudget::OauthAuth)),
        ApiPaths::OAUTH_BASE,
    )?;
    router = place(router, ApiPaths::OAUTH_BASE, public.merge(authenticated));
    router = place(router, crate::routes::sso::SSO_BASE, sso);
    Ok(router)
}
