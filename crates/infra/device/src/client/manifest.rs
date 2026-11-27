//! The signed manifest: its key, strict verification, and fetching with `ETag`
//! revalidation.
//!
//! The envelope's `payload` is verified with strict Ed25519 over its exact
//! UTF-8 bytes, as the instance signed them, before anything is parsed; it is
//! never re-canonicalised. Only then is the schema floor probed (so a newer
//! schema reports as incompatible rather than as an undecodable field) and the
//! manifest decoded. When a payload does not verify, the served key is fetched
//! again: a different key is a key change, the same key a bad signature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, VerifyingKey};
use reqwest::header::{CACHE_CONTROL, ETAG, HeaderValue, IF_NONE_MATCH};
use sp_protocol::names::decode_pubkey;
use sp_protocol::view::KeyFingerprint;
use sp_protocol::wire::auth::PubkeyResponse;
use sp_protocol::wire::manifest::{
    Manifest, SUPPORTED_SCHEMA_VERSION, SignedManifestEnvelope, probe_schema_version,
};
use sp_protocol::wire::paths;
use sp_protocol::{Jwt, PubkeyError};

use crate::client::client::SpClient;
use crate::client::error::{ClientError, Incompatibility};
use crate::client::http::{DEFAULT_TIMEOUT, SMALL_BODY, decode, decode_error, exchange, failure, protocol};

const MANIFEST_ENDPOINT: &str = "GET /v1/bridge/manifest";
const PUBKEY_ENDPOINT: &str = "GET /v1/bridge/pubkey";

const MANIFEST_BODY: usize = 32 * 1024 * 1024;

/// An instance's Ed25519 manifest key: the 32 raw bytes and their fingerprint.
#[derive(Clone, PartialEq, Eq)]
pub struct ManifestKey {
    raw: [u8; 32],
    fingerprint: KeyFingerprint,
}

impl ManifestKey {
    #[must_use]
    pub fn from_raw(raw: [u8; 32]) -> Self {
        Self {
            fingerprint: KeyFingerprint::of_key(&raw),
            raw,
        }
    }

    pub fn from_base64(encoded: &str) -> Result<Self, PubkeyError> {
        decode_pubkey(encoded).map(Self::from_raw)
    }

    #[must_use]
    pub const fn raw(&self) -> &[u8; 32] {
        &self.raw
    }

    #[must_use]
    pub fn to_base64(&self) -> String {
        STANDARD.encode(self.raw)
    }

    #[must_use]
    pub const fn fingerprint(&self) -> &KeyFingerprint {
        &self.fingerprint
    }
}

impl fmt::Debug for ManifestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ManifestKey")
            .field(&self.fingerprint.as_str())
            .finish()
    }
}

/// The answer to a manifest read: unchanged since the `ETag` given, or a
/// verified manifest with its envelope (whose `payload` is the exact signed
/// string, to persist and verify again later).
#[derive(Debug, Clone)]
pub enum ManifestFetch {
    NotModified,
    Fetched {
        envelope: SignedManifestEnvelope,
        manifest: Box<Manifest>,
        etag: Option<String>,
    },
}

pub fn verify(envelope: &SignedManifestEnvelope, key: &ManifestKey) -> Result<(), ClientError> {
    let invalid = || ClientError::SignatureInvalid {
        endpoint: MANIFEST_ENDPOINT,
    };
    let verifying = VerifyingKey::from_bytes(key.raw()).map_err(|_bad_key| invalid())?;
    let decoded = STANDARD
        .decode(envelope.signature.as_str().trim())
        .map_err(|_not_base64| invalid())?;
    let bytes = <[u8; 64]>::try_from(decoded.as_slice()).map_err(|_wrong_length| invalid())?;
    let signature = Signature::from_bytes(&bytes);
    verifying
        .verify_strict(envelope.payload.as_bytes(), &signature)
        .map_err(|_mismatch| invalid())
}

pub fn open_manifest(
    envelope: &SignedManifestEnvelope,
    key: &ManifestKey,
) -> Result<Manifest, ClientError> {
    verify(envelope, key)?;
    parse_verified(&envelope.payload)
}

fn parse_verified(payload: &str) -> Result<Manifest, ClientError> {
    let too_new = |found: u32| ClientError::Incompatible {
        endpoint: MANIFEST_ENDPOINT,
        what: Incompatibility::ManifestSchema { found },
    };
    if let Some(found) = probe_schema_version(payload)
        && found > SUPPORTED_SCHEMA_VERSION
    {
        return Err(too_new(found));
    }
    let manifest: Manifest = serde_json::from_str(payload)
        .map_err(|error| decode_error(MANIFEST_ENDPOINT, Some(200), &error))?;
    if manifest.schema_too_new() {
        return Err(too_new(manifest.min_schema_version));
    }
    Ok(manifest)
}

impl SpClient {
    pub async fn pubkey(&self) -> Result<ManifestKey, ClientError> {
        let request = self.get(self.at(paths::PUBKEY)?);
        let answer = exchange(PUBKEY_ENDPOINT, request, DEFAULT_TIMEOUT, SMALL_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(PUBKEY_ENDPOINT, &answer));
        }
        let response: PubkeyResponse = decode(PUBKEY_ENDPOINT, &answer)?;
        ManifestKey::from_base64(&response.pubkey).map_err(|_malformed| {
            protocol(
                PUBKEY_ENDPOINT,
                Some(200),
                "the manifest key is not 32 raw Ed25519 bytes",
            )
        })
    }

    pub async fn manifest(
        &self,
        jwt: &Jwt,
        key: &ManifestKey,
        etag: Option<&str>,
        no_cache: bool,
    ) -> Result<ManifestFetch, ClientError> {
        let mut request = self
            .get(self.at(paths::MANIFEST)?)
            .bearer_auth(jwt.expose());
        if let Some(tag) = etag.and_then(|tag| HeaderValue::from_str(tag).ok()) {
            request = request.header(IF_NONE_MATCH, tag);
        }
        if no_cache {
            request = request.header(CACHE_CONTROL, "no-cache");
        }
        let answer = exchange(MANIFEST_ENDPOINT, request, DEFAULT_TIMEOUT, MANIFEST_BODY).await?;
        match answer.status.as_u16() {
            304 => return Ok(ManifestFetch::NotModified),
            200 => {},
            _ => return Err(failure(MANIFEST_ENDPOINT, &answer)),
        }
        let etag = answer
            .headers
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let envelope: SignedManifestEnvelope = decode(MANIFEST_ENDPOINT, &answer)?;
        if let Err(refused) = verify(&envelope, key) {
            let served = self.pubkey().await?;
            if served.fingerprint() != key.fingerprint() {
                tracing::warn!(pinned = %key.fingerprint(), offered = %served.fingerprint(), "manifest key changed");
                return Err(ClientError::KeyChanged {
                    pinned: key.fingerprint().clone(),
                    offered: served.fingerprint().clone(),
                });
            }
            return Err(refused);
        }
        let manifest = parse_verified(&envelope.payload)?;
        Ok(ManifestFetch::Fetched {
            envelope,
            manifest: Box::new(manifest),
            etag,
        })
    }
}
