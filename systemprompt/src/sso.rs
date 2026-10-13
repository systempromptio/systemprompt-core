//! Browser-redirect single sign-on: the provider contract and identity
//! policy, the driver that runs registered providers, the registration
//! macros, and the HTTP projections a deployment uses to serve the flow from
//! its own paths.
//!
//! A provider is registered with [`register_sso_provider!`]; the API server
//! discovers it at startup and serves it under
//! [`SSO_BASE`]`/{id}/{start,callback,metadata}`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use systemprompt_api::routes::sso::{
    SSO_BASE, SsoRouteState, binder_from_headers, into_response, router, start_response,
};
pub use systemprompt_oauth_issuance::sso::{
    AuthEventSinkRegistration, BINDER_COOKIE_NAME, BrowserSessionSubject, CookieSpec,
    DirectoryOnlyRoleResolver, FinishOutcome, MintedSession, ResolutionPlan, ResolvedUser,
    RoleResolver, RoleResolverRegistration, SSO_STATE_TTL_SECS, SsoDriver, SsoHookContext,
    SsoProviderRegistration, StartOutcome, binder_from_cookie_header, mint_browser_session,
    permissions_for_roles, plan_resolution, sanitize_return_to,
};
pub use systemprompt_oauth_issuance::{
    register_auth_event_sink, register_role_resolver, register_sso_provider,
};
pub use systemprompt_traits::sso::{
    AuthEventSink, CookieSameSite, ExternalIdentity, GatedIdentity, ReplayGuard, SsoError,
    SsoIdentityPolicy, SsoLoginRefused, SsoLoginSuccess, SsoMetadataDoc, SsoPaths, SsoProtocol,
    SsoProvider, SsoRedirect, SsoRefusal, SsoReturn, SsoReturnBinding, SsoStart, SsoStateContext,
    SsoStateSeed,
};
