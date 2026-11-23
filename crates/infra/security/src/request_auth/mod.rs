//! The kernel's request authenticator.
//!
//! [`JwtRequestAuthenticator`] implements
//! [`systemprompt_models::auth::RequestAuthenticator`]: it validates the
//! bearer or session cookie (signature, issuer, audience), then the backing
//! user row (active, through a short-TTL cache), the session row
//! ([`attest_session`]) and the token's revocation, and builds the caller's
//! `RequestContext`. It resolves the context id from the `x-context-id`
//! header, or takes the one the request body named. A UI session sent as the
//! cookie is refused: it is a bearer.
//!
//! A kernel without a revocation store seeds [`UnavailableRevocations`]: every
//! token that carries a token id is then refused, never admitted unchecked.
//!
//! [`attest_session`] is the one predicate that a claimed session id is a
//! live session of the claimed user; the authenticator and the API's session
//! middleware both use it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod attestation;
mod authenticator;
pub mod context;
mod revocations;
pub mod users;

pub use attestation::{SessionAttestationError, attest_session};
pub use authenticator::{JwtRequestAuthenticator, JwtRequestAuthenticatorParts};
pub use revocations::UnavailableRevocations;
