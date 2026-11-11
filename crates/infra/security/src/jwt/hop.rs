//! Mint and verify the proxy-to-MCP-server hop token.
//!
//! [`mint_mcp_hop`] signs an [`McpHopIdentity`] for one server with the
//! deployment's active key, `typ` [`MCP_HOP_TOKEN_TYPE`], audience
//! [`mcp_hop_audience`] and a [`MCP_HOP_TTL`] expiry. [`decode_mcp_hop`]
//! verifies one against a [`VerificationKeyring`] (active, staged and live
//! retired keys), so a verifier needs only the public key set: RS256 with a
//! known `kid`, the hop `typ`, this deployment's issuer, exactly the one
//! server's audience, `exp`/`nbf` with [`MCP_HOP_LEEWAY_SECONDS`], a lifetime
//! no longer than [`MCP_HOP_MAX_LIFETIME`], and a `sub` equal to the identity
//! it carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use jsonwebtoken::{Algorithm, Header, Validation, decode, decode_header, encode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use systemprompt_identifiers::{AccessTokenId, McpServerId};
use systemprompt_models::auth::{
    MCP_HOP_MAX_LIFETIME, MCP_HOP_TOKEN_TYPE, MCP_HOP_TTL, McpHopClaims, McpHopIdentity,
    RegisteredClaims, mcp_hop_audience,
};

use super::validate::reject_critical_header;
use crate::error::{AuthError, AuthResult};
use crate::keys::{SigningKeys, VerificationKeyring};

pub const MCP_HOP_LEEWAY_SECONDS: u64 = 5;

#[must_use]
pub fn mcp_hop_claims(
    issuer: &str,
    server: &McpServerId,
    identity: &McpHopIdentity,
) -> McpHopClaims {
    let now = Utc::now().timestamp();
    McpHopClaims {
        registered: RegisteredClaims {
            sub: identity.user_id.to_string(),
            iat: now,
            exp: now.saturating_add(MCP_HOP_TTL.num_seconds()),
            nbf: Some(now),
            iss: issuer.to_owned(),
            aud: vec![mcp_hop_audience(server)],
            jti: AccessTokenId::generate().to_string(),
        },
        identity: identity.clone(),
    }
}

pub fn encode_mcp_hop(
    keys: &SigningKeys,
    claims: &McpHopClaims,
) -> Result<String, jsonwebtoken::errors::Error> {
    encode_typed(keys, MCP_HOP_TOKEN_TYPE, claims)
}

pub fn mint_mcp_hop(
    keys: &SigningKeys,
    issuer: &str,
    server: &McpServerId,
    identity: &McpHopIdentity,
) -> Result<String, jsonwebtoken::errors::Error> {
    encode_mcp_hop(keys, &mcp_hop_claims(issuer, server, identity))
}

pub fn decode_mcp_hop(
    keyring: &VerificationKeyring,
    issuer: &str,
    server: &McpServerId,
    token: &str,
) -> AuthResult<McpHopClaims> {
    let audience = mcp_hop_audience(server);
    let claims: McpHopClaims = decode_typed(
        keyring,
        &TypedPolicy {
            issuer,
            audience: audience.as_str(),
            typ: MCP_HOP_TOKEN_TYPE,
        },
        token,
    )?;
    let lifetime = claims.registered.exp.saturating_sub(claims.registered.iat);
    if lifetime <= 0 || lifetime > MCP_HOP_MAX_LIFETIME.num_seconds() {
        return Err(AuthError::McpHopLifetime { seconds: lifetime });
    }
    if claims.registered.sub != claims.identity.user_id.as_str() {
        return Err(AuthError::McpHopSubjectMismatch);
    }
    Ok(claims)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TypedPolicy<'a> {
    pub(crate) issuer: &'a str,
    pub(crate) audience: &'a str,
    pub(crate) typ: &'a str,
}

pub(crate) fn decode_typed<C: DeserializeOwned>(
    keyring: &VerificationKeyring,
    policy: &TypedPolicy<'_>,
    token: &str,
) -> AuthResult<C> {
    let header = decode_header(token).map_err(AuthError::InvalidToken)?;
    reject_critical_header(&header).map_err(AuthError::InvalidToken)?;
    if header.alg != Algorithm::RS256 {
        return Err(AuthError::UnsupportedAlgorithm {
            got: format!("{:?}", header.alg),
        });
    }
    if header.typ.as_deref() != Some(policy.typ) {
        return Err(AuthError::UnexpectedTokenType { got: header.typ });
    }
    let kid = header.kid.as_deref().ok_or(AuthError::MissingKid)?;
    let key = keyring
        .decoding_key_for_kid(kid)
        .ok_or_else(|| AuthError::UnknownKid(kid.to_owned()))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;
    validation.validate_nbf = true;
    validation.leeway = MCP_HOP_LEEWAY_SECONDS;
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.set_issuer(&[policy.issuer]);
    validation.set_audience(&[policy.audience]);

    decode::<C>(token, key, &validation)
        .map(|data| data.claims)
        .map_err(AuthError::InvalidToken)
}

pub(crate) fn encode_typed<C: Serialize>(
    keys: &SigningKeys,
    typ: &str,
    claims: &C,
) -> Result<String, jsonwebtoken::errors::Error> {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.active_kid().to_owned());
    header.typ = Some(typ.to_owned());
    encode(&header, claims, keys.encoding_key())
}
