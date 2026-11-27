//! Credential newtypes.
//!
//! Each wraps the raw string, prints as `<redacted>` in `Debug` and `Display`,
//! zeroizes its buffer on drop and hands the raw value out only through
//! `expose()`. They deserialize (an instance sends them) but never serialize: a
//! body that has to carry one borrows `expose()` explicitly, so no derived
//! `Serialize` can leak it into a log or a file. The kinds are the durable PAT,
//! the session JWT minted from it, the device link's one-time code, an enrolled
//! device's credential, the hook OAuth client's secret and a per-plugin hook
//! token.
//!
//! A pasted PAT or one-time code is trimmed; an empty credential is refused. A
//! device credential must be `sp_device_` plus `[A-Za-z0-9._~+/=-]+`, at most
//! 256 characters. A JWT's claims are read without verifying its signature,
//! only to schedule renewal and name the session; a JWT without a readable
//! `exp` counts as expired, so a token of unknown lifetime is never sent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::error::{JwtError, SecretError};

macro_rules! secret {
    ($name:ident, $kind:literal) => {
        #[derive(Clone, Zeroize, ZeroizeOnDrop)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn expose(&self) -> &str {
                &self.0
            }

            #[must_use]
            pub const fn len(&self) -> usize {
                self.0.len()
            }

            #[must_use]
            pub const fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("<redacted>")
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                Self::new(raw).map_err(serde::de::Error::custom)
            }
        }

        impl $name {
            const KIND: &'static str = $kind;
        }
    };
}

secret!(Pat, "personal access token");
secret!(Jwt, "session token");
secret!(OneTimeCode, "one-time code");
secret!(DeviceCredential, "device credential");
secret!(ClientSecret, "client secret");
secret!(HookToken, "hook token");

fn non_empty(raw: String, kind: &'static str) -> Result<String, SecretError> {
    if raw.trim().is_empty() {
        return Err(SecretError::Empty { kind });
    }
    Ok(raw)
}

fn trimmed(raw: String, kind: &'static str) -> Result<String, SecretError> {
    let raw = Zeroizing::new(raw);
    non_empty(raw.trim().to_owned(), kind)
}

impl Pat {
    pub fn new(raw: impl Into<String>) -> Result<Self, SecretError> {
        trimmed(raw.into(), Self::KIND).map(Self)
    }
}

impl OneTimeCode {
    pub fn new(raw: impl Into<String>) -> Result<Self, SecretError> {
        trimmed(raw.into(), Self::KIND).map(Self)
    }
}

impl ClientSecret {
    pub fn new(raw: impl Into<String>) -> Result<Self, SecretError> {
        non_empty(raw.into(), Self::KIND).map(Self)
    }
}

impl HookToken {
    pub fn new(raw: impl Into<String>) -> Result<Self, SecretError> {
        non_empty(raw.into(), Self::KIND).map(Self)
    }
}

pub const DEVICE_CREDENTIAL_MAX_LEN: usize = 256;

pub const DEVICE_CREDENTIAL_PREFIX: &str = "sp_device_";

impl DeviceCredential {
    pub fn new(raw: impl Into<String>) -> Result<Self, SecretError> {
        let mut raw = Zeroizing::new(non_empty(raw.into(), Self::KIND)?);
        let shaped = raw.len() <= DEVICE_CREDENTIAL_MAX_LEN
            && raw
                .strip_prefix(DEVICE_CREDENTIAL_PREFIX)
                .is_some_and(|rest| {
                    !rest.is_empty()
                        && rest
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"._~+/=-".contains(&b))
                });
        if !shaped {
            return Err(SecretError::Malformed { kind: Self::KIND });
        }
        Ok(Self(std::mem::take(&mut *raw)))
    }
}

impl Jwt {
    pub fn new(raw: impl Into<String>) -> Result<Self, SecretError> {
        non_empty(raw.into(), Self::KIND).map(Self)
    }

    #[must_use]
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        let claims = self.unverified_claims().ok()?;
        let exp = claims.get("exp")?;
        let seconds = exp
            .as_i64()
            .or_else(|| exp.as_f64().map(|value| value.floor() as i64))?;
        DateTime::<Utc>::from_timestamp(seconds, 0)
    }

    #[must_use]
    pub fn is_expired(&self, now: DateTime<Utc>, margin: chrono::Duration) -> bool {
        !self.outlives(now + margin)
    }

    fn outlives(&self, deadline: DateTime<Utc>) -> bool {
        self.expires_at().is_some_and(|exp| exp > deadline)
    }

    #[must_use]
    pub fn session_id(&self) -> Option<String> {
        self.unverified_claims()
            .ok()?
            .get("session_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    }

    // JSON: JWT claims are an open set; only `exp` and `session_id` are read from
    // it.
    pub fn unverified_claims(
        &self,
    ) -> Result<serde_json::Map<String, serde_json::Value>, JwtError> {
        let mut segments = self.0.split('.');
        let (Some(_header), Some(payload), Some(_signature), None) = (
            segments.next(),
            segments.next(),
            segments.next(),
            segments.next(),
        ) else {
            return Err(JwtError::Shape);
        };
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload.trim_end_matches('='))
            .map_err(JwtError::Base64)?;
        serde_json::from_slice(&bytes).map_err(JwtError::Claims)
    }
}
