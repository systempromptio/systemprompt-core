//! Browser-redirect single sign-on.
//!
//! Re-exports the provider contract and identity policy, the driver that runs
//! contributed providers, and the HTTP projections a deployment uses to serve
//! the flow from its own paths.
//!
//! A composed member contributes a provider; the API server serves it under
//! `{SSO_BASE}/{id}/start`, `/callback` and `/metadata` (see [`SSO_BASE`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use systemprompt_api::routes::sso::{
    SSO_BASE, SsoRouteState, binder_from_headers, into_response, router, start_response,
};
pub use systemprompt_oauth_issuance::sso::{
    BINDER_COOKIE_NAME, BrowserSessionSubject, CookieSpec, DirectoryOnlyRoleResolver,
    FinishOutcome, MintedSession, ResolvedUser, RoleResolver, SSO_STATE_TTL_SECS, SsoCallback,
    SsoDriver, SsoHookContext, SsoProviderError, StartOutcome, binder_from_cookie_header,
    mint_browser_session, permissions_for_roles, sanitize_return_to,
};
pub use systemprompt_traits::sso::{
    AuthEventSink, CookieSameSite, ExternalIdentity, GatedIdentity, ReplayGuard, SsoError,
    SsoIdentityPolicy, SsoLoginRefused, SsoLoginSuccess, SsoMetadataDoc, SsoPaths, SsoProtocol,
    SsoProvider, SsoRedirect, SsoRefusal, SsoReturn, SsoReturnBinding, SsoStart, SsoStateContext,
    SsoStateSeed,
};
