//! The public half of the deployment's token keys, with rotation overlap.
//!
//! [`TokenVerificationKeys`] is the verify-only description of the RS256
//! keys a token may be signed with: the active key, the staged key (rolled
//! out to every verifier before it is promoted to signer), and every retired
//! key, each valid for verification until its own `expires_at`. It holds no
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

impl RetiredJwk {
    #[must_use]
    pub fn is_live_at(&self, now: DateTime<Utc>) -> bool {
        now < self.expires_at
    }
}

/// The verify-only key set: the active RS256 key and the retired keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenVerificationKeys {
    pub active: Jwk,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staged: Option<Jwk>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retired: Vec<RetiredJwk>,
}

impl TokenVerificationKeys {
    pub fn retired_at(&self, now: DateTime<Utc>) -> impl Iterator<Item = &RetiredJwk> {
        self.retired
            .iter()
            .filter(move |retired| retired.is_live_at(now))
    }

    #[must_use]
    pub fn jwks_at(&self, now: DateTime<Utc>) -> Jwks {
        let mut keys = vec![self.active.clone()];
        keys.extend(self.staged.clone());
        keys.extend(self.retired_at(now).map(|retired| retired.jwk.clone()));
        Jwks { keys }
    }
}

/// [`TokenVerificationKeys`] with each member's decoding key built once.
#[derive(Clone)]
pub struct VerificationKeyring {
    keys: TokenVerificationKeys,
    active: DecodingKey,
    staged: Option<DecodingKey>,
    retired: Vec<DecodingKey>,
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
        let staged = keys
            .staged
            .as_ref()
            .map(|jwk| DecodingKey::from_rsa_components(&jwk.n, &jwk.e))
            .transpose()?;
        let retired = keys
            .retired
            .iter()
            .map(|retired| DecodingKey::from_rsa_components(&retired.jwk.n, &retired.jwk.e))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            keys,
            active,
            staged,
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
        if let (Some(jwk), Some(key)) = (&self.keys.staged, &self.staged)
            && jwk.kid == kid
        {
            return Some(key);
        }
        self.keys
            .retired
            .iter()
            .zip(&self.retired)
            .find(|(retired, _)| retired.jwk.kid == kid && retired.is_live_at(now))
            .map(|(_, key)| key)
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
