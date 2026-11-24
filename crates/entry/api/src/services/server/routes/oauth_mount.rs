//! Mounts the routes the `identity-http` member's
//! [`IdentitySurface`](systemprompt_oauth_issuance::surface::IdentitySurface)
//! builds. The member owns the handlers; this mount owns their layering.
//!
//! The public and authenticated OAuth routers and the SSO router are nested
//! under their `ApiPaths` bases. The authenticated router carries the
//! `RefuseFileManagedClientWrites` protection layer: while the `identity`
//! config plane is file-managed, client-management writes (`POST`/`PUT`/
//! `DELETE` under `/clients`) are refused with 409, because the declaration
//! under `services/identity/` is the source of truth. `/revoke` is its own
//! router, protected with the `UserUi` context so a `person_ui` client can
//! sign its own grant out; it is the one OAuth route that admits a person's UI
//! session, and every other authenticated OAuth route refuses one. The
//! passkey-link page is public under the `oauth_public` scope; the caller's
//! session revocation (`/api/v1/core/users`) is a user route under the
//! `oauth_auth` scope. API-key management is nested at `/api-keys` inside the
//! server's admin router (`/api/v1/admin`), so it shares that router's admin
//! gate and its one per-caller `admin` throttle. The OAuth discovery documents
//! are merged into the well-known router.
//!
//! Every route the surface builds is absent (404) when `identity-http` is not
//! admitted. When it is admitted, boot is refused with a typed
//! `IdentityMountError` if the identity surface is absent or was provided by
//! any member other than `identity-http`, or if the SSO providers are absent or
//! were built by any member other than `identity`, the slot's opener. Both are
//! read by type from the unscoped kernel state, so their provider is checked
//! rather than assumed: an install member cannot stand in for either.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::reserved::{CoreId, WEB_ROOT_BASE_PATH};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, MissingDependency, ProtectionLayer, RateBudget, RateLimit,
    RouteGroup, RouteProtection,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth_issuance::contract::SsoProviders;
use systemprompt_oauth_issuance::surface::{
    DynIdentitySurface, IdentityHostHandles, IdentityRouteState, IdentityRoutes,
};
use systemprompt_provider_contracts::DependencyProvider;
use systemprompt_traits::AppContext as _;

use super::RouteMountError;
use super::protocol::{MountCtx, declare};
use crate::routes::gateway::provided_by;
use crate::services::middleware::place;

const OAUTH_PUBLIC: RateLimit = RateLimit::new("oauth_public", RateBudget::OauthPublic);
const OAUTH_AUTH: RateLimit = RateLimit::new("oauth_auth", RateBudget::OauthAuth);

#[derive(Debug, thiserror::Error)]
pub(crate) enum IdentityMountError {
    #[error("identity-http is admitted but provides no identity surface")]
    MissingSurface,
    #[error("the identity surface is provided by {provider}, not identity-http")]
    ForeignSurface { provider: DependencyProvider },
    #[error("identity-http is admitted but the SSO slot was never built")]
    MissingSsoSlot(#[source] MissingDependency),
    #[error("the SSO providers are built by {provider}, not the identity member")]
    ForeignSsoSlot { provider: DependencyProvider },
}

const SSO_SLOT_OWNER: &str = "identity";

pub(super) fn identity_routes(
    mount: &MountCtx<'_>,
) -> Result<Option<IdentityRoutes>, RouteMountError> {
    let owner = ExtensionId::new(CoreId::IdentityHttp.id());
    let kernel = mount.composed.kernel;
    let Some(standing) = mount.composed.admitted.built().standing_of(&owner) else {
        return Ok(None);
    };
    let refused = |source: IdentityMountError| RouteMountError::initialization("identity", source);
    let surface = kernel
        .state()
        .find::<DynIdentitySurface>()
        .ok_or_else(|| refused(IdentityMountError::MissingSurface))?;
    provided_by::<DynIdentitySurface>(kernel, CoreId::IdentityHttp.id())
        .map_err(|provider| refused(IdentityMountError::ForeignSurface { provider }))?;
    let sso: Arc<SsoProviders> = kernel
        .resolved()
        .get()
        .map_err(|source| refused(IdentityMountError::MissingSsoSlot(source)))?;
    provided_by::<Arc<SsoProviders>>(kernel, SSO_SLOT_OWNER)
        .map_err(|provider| refused(IdentityMountError::ForeignSsoSlot { provider }))?;
    let ctx = mount.ctx;
    surface
        .build(IdentityRouteState {
            resolved: kernel.route_scoped(&owner, standing, &[]),
            host: IdentityHostHandles {
                config: Arc::clone(ctx.config_arc()),
                analytics: ctx.analytics_provider(),
                users: ctx.user_provider(),
                sso,
            },
        })
        .map(Some)
        .map_err(|source| RouteMountError::initialization("identity", source))
}

pub(super) struct IdentityOauthRoutes {
    pub(super) public: Router,
    pub(super) authenticated: Router,
    pub(super) revocation: Router,
    pub(super) sso: Router,
}

pub(super) fn mount_oauth(
    mut router: Router,
    mount: &MountCtx<'_>,
    identity: IdentityOauthRoutes,
) -> Result<Router, RouteMountError> {
    let sso = mount.protect(
        identity.sso,
        &RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
            .with_rate_limit(RateLimit::new("sso", RateBudget::OauthPublic)),
        ApiPaths::SSO_BASE,
    )?;
    let public = mount.protect(
        identity.public,
        &RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
            .with_rate_limit(OAUTH_PUBLIC),
        ApiPaths::OAUTH_BASE,
    )?;
    let authenticated = mount.protect(
        identity.authenticated,
        &RouteProtection::new(AuthzPolicy::user(), ContextKind::User)
            .with_layer(ProtectionLayer::RefuseFileManagedClientWrites)
            .with_rate_limit(OAUTH_AUTH),
        ApiPaths::OAUTH_BASE,
    )?;
    let revocation = mount.protect(
        identity.revocation,
        &RouteProtection::new(AuthzPolicy::user(), ContextKind::UserUi).with_rate_limit(OAUTH_AUTH),
        ApiPaths::OAUTH_BASE,
    )?;
    router = place(
        router,
        ApiPaths::OAUTH_BASE,
        public.merge(authenticated).merge(revocation),
    );
    Ok(place(router, ApiPaths::SSO_BASE, sso))
}

pub(super) fn mount_passkey_link_page(
    router: Router,
    mount: &MountCtx<'_>,
    page: Router,
) -> Result<Router, RouteMountError> {
    Ok(mount.mount(
        router,
        declare(
            page,
            WEB_ROOT_BASE_PATH,
            AuthzPolicy::public(),
            ContextKind::Public,
            RouteGroup::Oauth,
        )
        .with_rate_limit(OAUTH_PUBLIC),
    )?)
}

pub(super) fn mount_sessions(
    router: Router,
    mount: &MountCtx<'_>,
    sessions: Router,
) -> Result<Router, RouteMountError> {
    Ok(mount.mount(
        router,
        declare(
            sessions,
            ApiPaths::CORE_USERS,
            AuthzPolicy::user(),
            ContextKind::User,
            RouteGroup::ContentAndMisc,
        )
        .with_rate_limit(OAUTH_AUTH),
    )?)
}
