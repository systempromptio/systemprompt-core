//! OAuth and SSO route mounting.
//!
//! Builds the `OAuthState` (with `WebAuthn` when configured) and nests the
//! public and authenticated OAuth routers and the SSO router under their
//! `ApiPaths` bases. While the `identity` config plane is file-managed, the
//! authenticated client-management writes (`POST`/`PUT`/`DELETE` under
//! `/clients`) are refused with 409: the declaration under
//! `services/identity/` is the source of truth.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::ApiError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::services::WebAuthnService;
use systemprompt_oauth::{OAuthSigning, OAuthState};
use systemprompt_oauth_issuance::{IDENTITY_PLANE, JwksCache};
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as AppContextTrait;

use super::protocol::MountCtx;
use crate::error::ApiHttpError;
use crate::services::middleware::RouterExt;
use crate::services::middleware::authz::AuthzPolicy;

fn create_oauth_state(ctx: &AppContext) -> Result<OAuthState, LoaderError> {
    let missing = |provider: &str| LoaderError::InitializationFailed {
        extension: ExtensionId::new("oauth"),
        message: format!("{provider} is required to mount the OAuth routes"),
    };
    let analytics = ctx
        .analytics_provider()
        .ok_or_else(|| missing("AnalyticsProvider"))?;
    let users = ctx.user_provider().ok_or_else(|| missing("UserProvider"))?;
    let sessions = ctx
        .session_provider()
        .ok_or_else(|| missing("SessionProvider"))?;
    let mcp_registry: Arc<dyn systemprompt_traits::McpRegistryProvider> =
        Arc::new(ctx.mcp_registry().clone());
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
    )
    .with_mcp_registry(mcp_registry);
    match webauthn {
        Ok(service) => Ok(state.with_webauthn(Arc::new(service))),
        Err(e) => {
            tracing::warn!(error = %e, "WebAuthn is not configured; passkey routes will refuse");
            Ok(state)
        },
    }
}

const CLIENTS_PATH: &str = "/clients";

async fn refuse_file_managed_clients(ctx: AppContext, req: Request, next: Next) -> Response {
    if req.uri().path().starts_with(CLIENTS_PATH) && !req.method().is_safe() {
        let refused = ctx
            .config_planes()
            .and_then(|planes| planes.writer(IDENTITY_PLANE).map(|_| ()));
        if let Err(error) = refused {
            return ApiHttpError::from(ApiError::from(error)).into_response();
        }
    }
    next.run(req).await
}

pub(super) fn mount_oauth(mut router: Router, mount: &MountCtx<'_>) -> Result<Router, LoaderError> {
    let (ctx, limits, public_middleware, user_middleware) = (
        mount.ctx,
        mount.limits,
        mount.public_middleware,
        mount.user_middleware,
    );
    let rate_config = &ctx.config().rate_limits;
    let oauth_state = create_oauth_state(ctx)?;
    let sso_state = crate::routes::sso::SsoRouteState::build(ctx, &oauth_state);
    let sso = crate::routes::sso::router(sso_state)
        .with_rate_limit(limits, rate_config.oauth_public_per_second, "sso")?
        .with_auth(*public_middleware, AuthzPolicy::public());
    let oauth = crate::routes::oauth::public_router(Arc::new(JwksCache::new()))
        .with_state(oauth_state.clone())
        .with_rate_limit(limits, rate_config.oauth_public_per_second, "oauth_public")?
        .with_auth(*public_middleware, AuthzPolicy::public())
        .merge(
            crate::routes::oauth::authenticated_router()
                .with_state(oauth_state)
                .layer(axum::middleware::from_fn({
                    let ctx = ctx.clone();
                    move |req: Request, next: Next| {
                        refuse_file_managed_clients(ctx.clone(), req, next)
                    }
                }))
                .with_rate_limit(limits, rate_config.oauth_auth_per_second, "oauth_auth")?
                .with_auth(user_middleware.clone(), AuthzPolicy::user()),
        );
    router = router.nest(ApiPaths::OAUTH_BASE, oauth);
    router = router.nest(crate::routes::sso::SSO_BASE, sso);
    Ok(router)
}
