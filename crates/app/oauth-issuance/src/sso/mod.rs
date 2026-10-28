//! Browser-redirect single sign-on orchestration.
//!
//! [`SsoDriver`] runs every provider registered with
//! [`register_sso_provider!`](crate::register_sso_provider): `start` stores
//! the attempt and binds it to the browser with a cookie; `finish` correlates
//! the return, lets the provider validate it, enforces replay, the identity
//! policy and provisioning, mints the browser session and audits the outcome
//! to every [`AuthEventSink`](systemprompt_traits::sso::AuthEventSink).
//!
//! [`oidc`] is the one provider core ships: a generic `OpenID Connect`
//! relying party, off unless `services/web/config/oidc.yaml` enables it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cookie;
mod driver;
mod finish;
pub mod oidc;
mod registry;
mod resolve;
mod roles;
mod session;
mod start;

pub use cookie::{BINDER_COOKIE_NAME, CookieSpec, binder_from_cookie_header};
pub use driver::{FinishOutcome, SsoDriver, StartOutcome};
pub use finish::SsoCallback;
pub use registry::{
    AuthEventSinkRegistration, SsoHookContext, SsoProviderError, SsoProviderRegistration,
};
pub use resolve::{ResolutionPlan, ResolvedUser, plan_resolution};
pub use roles::{DirectoryOnlyRoleResolver, RoleResolver, RoleResolverRegistration};
pub use session::{
    BrowserSessionSubject, MintedSession, mint_browser_session, permissions_for_roles,
};
pub use start::{SSO_STATE_TTL_SECS, sanitize_return_to};
pub use systemprompt_traits::sso::CookieSameSite;
