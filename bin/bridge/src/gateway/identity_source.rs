//! The seam that lets a white-label bridge point whoami at its own endpoint.
//!
//! The stock gateway answers `GET /v1/bridge/whoami` with the subset it can
//! prove from the JWT and the user row — id, email, display name, roles. A
//! deployment whose identity provider knows more (federated issuer, AD groups,
//! organization, department) can serve a richer envelope of its own and
//! register its path here; the extra keys ride through
//! [`crate::gateway::types::WhoamiResponse::extra`] to the profile tab
//! untouched.
//!
//! Registration is compile-time [`inventory`], not a `Brand` field, so adding
//! this seam does not force every existing brand literal to grow a member.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Where this build fetches its identity envelope from.
#[derive(Debug, Clone, Copy)]
pub struct IdentitySourceRegistration {
    pub whoami_path: &'static str,
}

inventory::collect!(IdentitySourceRegistration);

pub const DEFAULT_WHOAMI_PATH: &str = "/v1/bridge/whoami";

// Why: only one registration is meaningful; a second is ignored rather than
// treated as an error, because a binary that links two of them is a build
// mistake that must not take the sign-in flow down with it.
#[must_use]
pub fn whoami_path() -> &'static str {
    inventory::iter::<IdentitySourceRegistration>
        .into_iter()
        .next()
        .map_or(DEFAULT_WHOAMI_PATH, |reg| reg.whoami_path)
}

#[macro_export]
macro_rules! register_identity_source {
    ($path:expr $(,)?) => {
        ::inventory::submit! {
            $crate::gateway::identity_source::IdentitySourceRegistration { whoami_path: $path }
        }
    };
}
