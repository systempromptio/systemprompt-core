//! The single RS256 decode primitive shared by every JWT validation path.
//!
//! Request-context middleware, session validation, hook-token validation, and
//! the OAuth/MCP/agent domains all route through [`decode_rs256_claims`]. The
//! `kid` lookup, RS256 enforcement, and the `exp`/`nbf`/issuer/audience policy
//! live here and nowhere else, so the validators cannot drift apart. The only
//! per-call knob is [`ValidationPolicy`], and every policy pins an issuer.
//!
//! Session tokens — the main API middleware and the A2A server alike — go
//! through [`decode_session_claims`], which adds the act-chain depth limit and
//! re-derives `user_type` from `scope`, so a forged or mis-minted type claim or
//! an unbounded delegation chain cannot ride either path.
//!
//! Audience validation is always on: every policy carries a non-empty expected
//! audience set, and a policy that reaches the decoder with an empty set is
//! rejected as [`AuthError::EmptyAudiencePolicy`] rather than silently
//! accepting any `aud`. [`ValidationPolicy::session_context`] pins
//! [`JwtAudience::FIRST_PARTY`], so a token minted for a narrower surface
//! (`hook`, a custom resource) cannot ride the session middleware.
//!
//! Federated subject-token verification (token-exchange) is deliberately *not*
//! a caller: it resolves keys from an external issuer's JWKS rather than this
//! deployment's signing authority, so it is a genuinely different operation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use systemprompt_models::auth::{JwtAudience, JwtClaims, MAX_ACT_CHAIN_DEPTH, UserType};

use crate::error::{AuthError, AuthResult};
use crate::keys::authority;

pub const JWT_LEEWAY_SECONDS: u64 = 30;

#[derive(Debug, Clone)]
pub struct ValidationPolicy<'a> {
    leeway_seconds: u64,
    issuer: &'a str,
    audiences: &'a [JwtAudience],
}

impl<'a> ValidationPolicy<'a> {
    #[must_use]
    pub const fn session_context(issuer: &'a str) -> Self {
        Self::issuer_scoped(issuer, JwtAudience::FIRST_PARTY)
    }

    #[must_use]
    pub const fn issuer_scoped(issuer: &'a str, audiences: &'a [JwtAudience]) -> Self {
        Self {
            leeway_seconds: JWT_LEEWAY_SECONDS,
            issuer,
            audiences,
        }
    }
}

pub fn decode_rs256_claims(token: &str, policy: &ValidationPolicy<'_>) -> AuthResult<JwtClaims> {
    if policy.audiences.is_empty() {
        return Err(AuthError::EmptyAudiencePolicy);
    }

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
    validation.validate_exp = true;
    validation.validate_nbf = true;
    validation.leeway = policy.leeway_seconds;
    validation.set_issuer(&[policy.issuer]);
    let audience_strs: Vec<&str> = policy.audiences.iter().map(JwtAudience::as_str).collect();
    validation.set_audience(&audience_strs);

    decode::<JwtClaims>(token, key, &validation)
        .map(|data| data.claims)
        .map_err(AuthError::InvalidToken)
}

pub fn decode_session_claims(token: &str, policy: &ValidationPolicy<'_>) -> AuthResult<JwtClaims> {
    let claims = decode_rs256_claims(token, policy)?;

    if let Some(act) = claims.act.as_ref() {
        let depth = act.depth();
        if depth > MAX_ACT_CHAIN_DEPTH {
            return Err(AuthError::ActChainTooDeep {
                depth,
                max: MAX_ACT_CHAIN_DEPTH,
            });
        }
    }

    let derived_type = UserType::from_permissions(&claims.scope);
    if derived_type != claims.user_type {
        return Err(AuthError::UserTypeMismatch {
            claimed: claims.user_type,
            derived: derived_type,
        });
    }

    Ok(claims)
}
