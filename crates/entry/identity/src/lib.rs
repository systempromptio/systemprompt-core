//! `systemprompt-identity-entry` — the identity entry surface.
//!
//! The `http` feature carries the identity routes: OAuth 2.0 / OIDC, SSO, the
//! caller's own session revocation and admin API-key management
//! ([`http`]). The crate's composition member ([`extension`]) contributes
//! them as an
//! [`IdentitySurface`](systemprompt_oauth_issuance::surface::IdentitySurface)
//! the API mount builds and layers, so the API server does not name this
//! crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod extension;
#[cfg(feature = "http")]
pub mod http;
