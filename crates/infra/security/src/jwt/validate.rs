//! The single RS256 decode primitive shared by every JWT validation path.
//!
//! Request-context middleware, session validation, hook-token validation, and
//! the OAuth/MCP/agent domains all route through [`decode_rs256_claims`]. The
//! `kid` lookup, RS256 enforcement, and the `exp`/`nbf`/issuer/audience policy
//! live here and nowhere else, so the validators cannot drift apart. The only
//! per-call knob is [`ValidationPolicy`].
//!
//! Federated subject-token verification (token-exchange) is deliberately *not*
//! a caller: it resolves keys from an external issuer's JWKS rather than this
//! deployment's signing authority, so it is a genuinely different operation.

use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use systemprompt_models::auth::{JwtAudience, JwtClaims};

use crate::error::{AuthError, AuthResult};
use crate::keys::authority;

/// Clock-skew tolerance (seconds) for `exp`/`nbf`/`iat`. Pinned explicitly so
/// deployments see the value in review rather than inheriting the
/// `jsonwebtoken` default.
pub const JWT_LEEWAY_SECONDS: u64 = 30;

/// The claim checks applied on top of the always-on signature, RS256, and
/// `kid` enforcement. An empty `audiences` slice disables the `aud` check.
#[derive(Debug, Clone, Default)]
pub struct ValidationPolicy<'a> {
    pub validate_exp: bool,
    pub validate_nbf: bool,
    pub leeway_seconds: u64,
    pub issuer: Option<&'a str>,
    pub audiences: &'a [JwtAudience],
}

impl<'a> ValidationPolicy<'a> {
    /// Stateless decode for request-context middleware that performs its own
    /// DB-backed session and user checks after decode. Validates `exp` and
    /// `nbf` (with leeway); issuer and audience are enforced by the stateful
    /// validators that hold deployment config, not here.
    #[must_use]
    pub const fn session_context() -> Self {
        Self {
            validate_exp: true,
            validate_nbf: true,
            leeway_seconds: JWT_LEEWAY_SECONDS,
            issuer: None,
            audiences: &[],
        }
    }

    /// Full first-party validation: `exp` + `nbf` + issuer pinning + audience
    /// membership, with the standard leeway.
    #[must_use]
    pub const fn issuer_scoped(issuer: &'a str, audiences: &'a [JwtAudience]) -> Self {
        Self {
            validate_exp: true,
            validate_nbf: true,
            leeway_seconds: JWT_LEEWAY_SECONDS,
            issuer: Some(issuer),
            audiences,
        }
    }
}

pub fn decode_rs256_claims(token: &str, policy: &ValidationPolicy<'_>) -> AuthResult<JwtClaims> {
    let header = decode_header(token).map_err(AuthError::InvalidToken)?;
    if header.alg != Algorithm::RS256 {
        return Err(AuthError::UnsupportedAlgorithm {
            got: format!("{:?}", header.alg),
        });
    }
    let kid = header.kid.as_deref().ok_or(AuthError::MissingKid)?;
    let key = authority::decoding_key_for_kid(kid)
        .map_err(|e| AuthError::KeyLookup(e.to_string()))?
        .ok_or_else(|| AuthError::UnknownKid(kid.to_owned()))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = policy.validate_exp;
    validation.validate_nbf = policy.validate_nbf;
    validation.leeway = policy.leeway_seconds;
    if let Some(issuer) = policy.issuer {
        validation.set_issuer(&[issuer]);
    }
    if policy.audiences.is_empty() {
        validation.validate_aud = false;
    } else {
        let audience_strs: Vec<&str> = policy.audiences.iter().map(JwtAudience::as_str).collect();
        validation.set_audience(&audience_strs);
    }

    decode::<JwtClaims>(token, key, &validation)
        .map(|data| data.claims)
        .map_err(AuthError::InvalidToken)
}
