//! The signed marker on a request one replica forwards to the replica that
//! owns a process.
//!
//! A replica that receives a request carrying a marker it can verify serves
//! it locally instead of forwarding again; a marker it cannot verify (absent,
//! client-supplied, expired, for another service) is ignored and the request
//! is routed as if it had arrived directly. The marker is an RS256 token with
//! `typ` [`OWNER_HOP_TOKEN_TYPE`], audience `owner-hop:<service>` and an
//! [`OWNER_HOP_TTL`] expiry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AccessTokenId, ServiceName};

use super::hop::{TypedPolicy, decode_typed, encode_typed};
use crate::error::{AuthResult, JwtError};
use crate::keys::{SigningKeys, VerificationKeyring};

pub const OWNER_HOP_TOKEN_TYPE: &str = "owner-hop+jwt";

pub const OWNER_HOP_TTL: Duration = Duration::seconds(30);

const OWNER_HOP_SUBJECT: &str = "owner-hop";

/// Claims of an owner-hop marker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerHopClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: i64,
    pub nbf: i64,
    pub exp: i64,
    pub jti: AccessTokenId,
}

#[must_use]
pub fn owner_hop_audience(service: &ServiceName) -> String {
    format!("owner-hop:{}", service.as_str())
}

pub fn mint_owner_hop(
    keys: &SigningKeys,
    issuer: &str,
    service: &ServiceName,
) -> Result<String, JwtError> {
    let now = Utc::now().timestamp();
    let claims = OwnerHopClaims {
        iss: issuer.to_owned(),
        sub: OWNER_HOP_SUBJECT.to_owned(),
        aud: owner_hop_audience(service),
        iat: now,
        nbf: now,
        exp: now.saturating_add(OWNER_HOP_TTL.num_seconds()),
        jti: AccessTokenId::generate(),
    };
    Ok(encode_typed(keys, OWNER_HOP_TOKEN_TYPE, &claims)?)
}

pub fn verify_owner_hop(
    keyring: &VerificationKeyring,
    issuer: &str,
    service: &ServiceName,
    token: &str,
) -> AuthResult<OwnerHopClaims> {
    let audience = owner_hop_audience(service);
    decode_typed(
        keyring,
        &TypedPolicy {
            issuer,
            audience: &audience,
            typ: OWNER_HOP_TOKEN_TYPE,
        },
        token,
    )
}
