//! The public half of the deployment's token keys, with rotation overlap.
//!
//! [`TokenVerificationKeys`] is the verify-only description of the RS256
//! keys a token may be signed with: the active key, and at most one retired
//! key that stays valid for verification until its `expires_at`. It holds no
//! private material, serialises to JSON, and is what a process that only
//! verifies tokens (an MCP subprocess, for one) receives in place of the
//! signing PEM. [`VerificationKeyring`] is the same set with its decoding keys
//! built once, and is what every verifier consults by `kid`.
//!
//! A retired key past its expiry is neither accepted nor published: from that
//! instant a token signed with it fails as an unknown `kid`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use jsonwebtoken::DecodingKey;
use serde::{Deserialize, Serialize};

use crate::keys::{Jwk, Jwks};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetiredJwk {
    pub jwk: Jwk,
    pub expires_at: DateTime<Utc>,
}

/// The verify-only key set: the active RS256 key and an optional retired key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenVerificationKeys {
    pub active: Jwk,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired: Option<RetiredJwk>,
}

impl TokenVerificationKeys {
    #[must_use]
    pub fn retired_at(&self, now: DateTime<Utc>) -> Option<&Jwk> {
        self.retired
            .as_ref()
            .filter(|retired| now < retired.expires_at)
            .map(|retired| &retired.jwk)
    }

    #[must_use]
    pub fn jwks_at(&self, now: DateTime<Utc>) -> Jwks {
        let mut keys = vec![self.active.clone()];
        keys.extend(self.retired_at(now).cloned());
        Jwks { keys }
    }
}

/// [`TokenVerificationKeys`] with each member's decoding key built once.
#[derive(Clone)]
pub struct VerificationKeyring {
    keys: TokenVerificationKeys,
    active: DecodingKey,
    retired: Option<DecodingKey>,
}

impl std::fmt::Debug for VerificationKeyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerificationKeyring")
            .field("keys", &self.keys)
            .finish_non_exhaustive()
    }
}

impl VerificationKeyring {
    pub fn new(keys: TokenVerificationKeys) -> Result<Self, jsonwebtoken::errors::Error> {
        let active = DecodingKey::from_rsa_components(&keys.active.n, &keys.active.e)?;
        let retired = keys
            .retired
            .as_ref()
            .map(|retired| DecodingKey::from_rsa_components(&retired.jwk.n, &retired.jwk.e))
            .transpose()?;
        Ok(Self {
            keys,
            active,
            retired,
        })
    }

    #[must_use]
    pub const fn keys(&self) -> &TokenVerificationKeys {
        &self.keys
    }

    #[must_use]
    pub fn decoding_key_for_kid_at(&self, kid: &str, now: DateTime<Utc>) -> Option<&DecodingKey> {
        if self.keys.active.kid == kid {
            return Some(&self.active);
        }
        match (self.keys.retired_at(now), self.retired.as_ref()) {
            (Some(jwk), Some(key)) if jwk.kid == kid => Some(key),
            _ => None,
        }
    }

    #[must_use]
    pub fn decoding_key_for_kid(&self, kid: &str) -> Option<&DecodingKey> {
        self.decoding_key_for_kid_at(kid, Utc::now())
    }

    #[must_use]
    pub fn jwks(&self) -> Jwks {
        self.keys.jwks_at(Utc::now())
    }
}
