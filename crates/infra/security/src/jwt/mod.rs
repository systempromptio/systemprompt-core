//! JWT plane.
//!
//! Two stateless surfaces, each a free function or unit-struct method that
//! never holds JWT state of its own:
//!
//! - [`mint`] — issues administrator-scoped RS256 tokens via
//!   [`JwtService::generate_admin_token`]. Session-scoped tokens are minted by
//!   [`crate::session::SessionGenerator`] instead.
//! - [`decode`] — turns a raw `Bearer …` string into a typed
//!   [`systemprompt_models::auth::JwtUserContext`] through
//!   [`validate::decode_session_claims`]: kid + RS256, the deployment issuer, a
//!   first-party `aud`, the act-chain depth limit, and `user_type` re-derived
//!   from `scope` (defence-in-depth against a forged claim), surfacing every
//!   failure as an [`crate::AuthError`] variant.
//!
//! [`decode::extract_user_context`] serves the request authenticator, which
//! does the session, user and revocation lookups against the database after
//! decode.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod audience;
pub mod authority;
pub mod authorization;
pub mod decode;
pub mod hop;
pub mod lifetime;
pub mod mint;
pub mod owner_hop;
pub mod scoped;
pub mod validate;
pub mod verifying;

pub use audience::{validate_any_audience, validate_required_audience, validate_service_access};
pub use authority::{DeploymentTokenAuthority, encode_rs256, user_claims};
pub use authorization::AuthorizationService;
pub use decode::{extract_user_context, extract_user_context_for};
pub use hop::{
    MCP_HOP_LEEWAY_SECONDS, decode_mcp_hop, encode_mcp_hop, mcp_hop_claims, mint_mcp_hop,
};
pub use lifetime::{
    MAX_TOKEN_LIFETIME, RotationOverlapError, SignedTokenKind, SignedTokenLifetime, cut_off_by,
    required_overlap, rotation_overlap, signed_token_lifetimes, within_max_lifetime,
};
pub use mint::{AdminTokenParams, JwtService};
pub use owner_hop::{
    OWNER_HOP_TOKEN_TYPE, OWNER_HOP_TTL, OwnerHopClaims, mint_owner_hop, owner_hop_audience,
    verify_owner_hop,
};
pub use scoped::{SCOPED_TOKEN_TYPE, ScopedTokenClaims, decode_scoped_token};
pub use validate::{
    JWT_LEEWAY_SECONDS, ValidationPolicy, decode_rs256_claims, decode_session_claims,
    reject_critical_header,
};
pub use verifying::{SigningUnavailable, VerifyingTokenAuthority};
