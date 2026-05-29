//! OAuth 2.0 / OIDC HTTP surface.
//!
//! Assembles the OAuth router ([`core`]) over the protocol [`endpoints`],
//! dynamic client management ([`client`], [`clients`]), [`discovery`] and
//! [`wellknown`] metadata, [`webauthn`] passkey flows, and the shared
//! [`OAuthHttpError`] response model ([`error`]).

pub mod client;
pub mod clients;
pub mod core;
pub mod discovery;
pub mod endpoints;
pub mod error;
pub mod extractors;
pub mod health;
pub mod responses;
pub mod webauthn;
pub mod wellknown;

pub use core::{authenticated_router, public_router, router};
pub use error::{OAuthErrorCode, OAuthHttpError};
pub use health::*;
pub use wellknown::wellknown_routes;
