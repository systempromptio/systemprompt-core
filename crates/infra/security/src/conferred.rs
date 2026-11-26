//! The core issuers of the member cipher and the member share-token signer,
//! the conferred capabilities that use the deployment's sealed key material
//! without exporting it.
//!
//! [`MemberCiphers`] holds the `encryption_master_key`. Each member's cipher
//! is ChaCha20-Poly1305 (the [`AtRestCipher`]) under
//! HMAC-SHA-256(master key, `systemprompt.member-cipher.v1` NUL member id):
//! a separate key per member and separate from the key core seals with, so a
//! member's handle opens only its own ciphertext. A sealed value carries the
//! `spmc1.` envelope, which is how `admin secrets reseal` tells a value
//! already under a member cipher from raw-master-key ciphertext.
//!
//! [`MemberShareTokens`] holds the `manifest_signing_secret_seed`. A token is
//! `spst1.<base64url(expiry ‖ payload)>.<base64url(tag)>`, where the tag is
//! HMAC-SHA-256(seed, `systemprompt.member-share-token.v1` NUL member id NUL
//! purpose NUL body): bound to the member, the purpose and the expiry. The
//! token is not a JWT and its key is not the manifest signing key, so no core
//! verifier accepts it, and one member's signer verifies nothing another member
//! signed.
//!
//! [`BrokerCallVerifiers`] holds a keyed digest of the
//! `mcp_credential_broker_secret`, never the secret: HMAC-SHA-256 under the
//! label `systemprompt.broker-call.v1`. A member's verifier digests the
//! presented `x-systemprompt-credential-broker` header the same way and
//! compares in constant time, so it answers whether a call is core's broker
//! and can neither reveal the secret nor present it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::{
    BrokerCallRefused, BrokerCallVerifier, ConferralIssuer, MemberCipher, MemberCipherError,
    MemberGrant, SHARE_TOKEN_MAX_TTL, ShareTokenError, ShareTokenSigner,
};

use crate::at_rest::{AtRestCipher, AtRestCipherError, AtRestHashError, MasterKey, keyed_hmac};

const SHARE_TOKEN_LABEL: &[u8] = b"systemprompt.member-share-token.v1";
const SHARE_TOKEN_PREFIX: &str = "spst1";
const EXPIRY_BYTES: usize = 8;
const PURPOSE_MAX: usize = 64;

fn keyed(root: &[u8; 32]) -> Result<Hmac<Sha256>, AtRestHashError> {
    keyed_hmac(root)
}

fn labelled(root: &Hmac<Sha256>, label: &[u8], member: &ExtensionId) -> Hmac<Sha256> {
    let mut mac = root.clone();
    mac.update(label);
    mac.update(&[0]);
    mac.update(member.as_str().as_bytes());
    mac.update(&[0]);
    mac
}

/// The sealed issuer of every member's cipher.
pub struct MemberCiphers {
    master_key: MasterKey,
}

impl MemberCiphers {
    pub fn from_master_key(master_key: &MasterKey) -> Result<Self, AtRestHashError> {
        keyed(master_key.bytes())?;
        Ok(Self {
            master_key: master_key.clone(),
        })
    }
}

impl std::fmt::Debug for MemberCiphers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemberCiphers(<redacted>)")
    }
}

impl ConferralIssuer<dyn MemberCipher> for MemberCiphers {
    fn issue(&self, grant: &MemberGrant) -> Arc<dyn MemberCipher> {
        let cipher = AtRestCipher::for_member(&self.master_key, &grant.member)
            .unwrap_or_else(|_unkeyable| AtRestCipher::withheld());
        Arc::new(MemberCipherHandle(cipher))
    }
}

#[derive(Debug)]
struct MemberCipherHandle(AtRestCipher);

const fn cipher_error(error: AtRestCipherError) -> MemberCipherError {
    match error {
        AtRestCipherError::Malformed | AtRestCipherError::Unresealed => {
            MemberCipherError::Malformed
        },
        AtRestCipherError::NotAuthentic
        | AtRestCipherError::KeyUnavailable
        | AtRestCipherError::KeyInvalid => MemberCipherError::NotAuthentic,
    }
}

impl MemberCipher for MemberCipherHandle {
    fn seal(&self, plaintext: &[u8]) -> Result<String, MemberCipherError> {
        self.0.seal_bytes(plaintext).map_err(cipher_error)
    }

    fn open(&self, sealed: &str) -> Result<Vec<u8>, MemberCipherError> {
        self.0.open_bytes(sealed).map_err(cipher_error)
    }
}

/// The sealed issuer of every member's share-token signer.
pub struct MemberShareTokens {
    root: Hmac<Sha256>,
}

impl MemberShareTokens {
    pub fn from_seed(seed: &[u8; 32]) -> Result<Self, AtRestHashError> {
        Ok(Self { root: keyed(seed)? })
    }
}

impl std::fmt::Debug for MemberShareTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemberShareTokens(<redacted>)")
    }
}

impl ConferralIssuer<dyn ShareTokenSigner> for MemberShareTokens {
    fn issue(&self, grant: &MemberGrant) -> Arc<dyn ShareTokenSigner> {
        Arc::new(MemberShareTokenSigner {
            member: labelled(&self.root, SHARE_TOKEN_LABEL, &grant.member),
        })
    }
}

struct MemberShareTokenSigner {
    member: Hmac<Sha256>,
}

impl std::fmt::Debug for MemberShareTokenSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemberShareTokenSigner(<redacted>)")
    }
}

fn check_purpose(purpose: &str) -> Result<(), ShareTokenError> {
    let valid = !purpose.is_empty()
        && purpose.len() <= PURPOSE_MAX
        && purpose.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(ShareTokenError::Purpose)
    }
}

impl MemberShareTokenSigner {
    fn mac(&self, purpose: &str, body: &[u8]) -> Hmac<Sha256> {
        let mut mac = self.member.clone();
        mac.update(purpose.as_bytes());
        mac.update(&[0]);
        mac.update(body);
        mac
    }
}

impl ShareTokenSigner for MemberShareTokenSigner {
    fn sign(
        &self,
        purpose: &str,
        payload: &[u8],
        ttl: Duration,
    ) -> Result<String, ShareTokenError> {
        check_purpose(purpose)?;
        if ttl < Duration::from_secs(1) || ttl > SHARE_TOKEN_MAX_TTL {
            return Err(ShareTokenError::Lifetime);
        }
        let ttl_secs = i64::try_from(ttl.as_secs()).map_err(|_e| ShareTokenError::Lifetime)?;
        let expiry = chrono::Utc::now().timestamp().saturating_add(ttl_secs);
        let mut body = Vec::with_capacity(EXPIRY_BYTES + payload.len());
        body.extend_from_slice(&expiry.to_be_bytes());
        body.extend_from_slice(payload);
        let tag = self.mac(purpose, &body).finalize().into_bytes();
        Ok(format!(
            "{SHARE_TOKEN_PREFIX}.{}.{}",
            URL_SAFE_NO_PAD.encode(&body),
            URL_SAFE_NO_PAD.encode(tag)
        ))
    }

    fn verify(&self, purpose: &str, token: &str) -> Result<Vec<u8>, ShareTokenError> {
        check_purpose(purpose)?;
        let mut parts = token.split('.');
        let (Some(SHARE_TOKEN_PREFIX), Some(body), Some(tag), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(ShareTokenError::Malformed);
        };
        let body = URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_e| ShareTokenError::Malformed)?;
        let tag = URL_SAFE_NO_PAD
            .decode(tag)
            .map_err(|_e| ShareTokenError::Malformed)?;
        if body.len() < EXPIRY_BYTES {
            return Err(ShareTokenError::Malformed);
        }
        self.mac(purpose, &body)
            .verify_slice(&tag)
            .map_err(|_e| ShareTokenError::NotAuthentic)?;
        let (expiry, payload) = body.split_at(EXPIRY_BYTES);
        let expiry =
            i64::from_be_bytes(expiry.try_into().map_err(|_e| ShareTokenError::Malformed)?);
        if chrono::Utc::now().timestamp() >= expiry {
            return Err(ShareTokenError::Expired);
        }
        Ok(payload.to_vec())
    }
}

const BROKER_CALL_LABEL: &[u8] = b"systemprompt.broker-call.v1";

/// The sealed issuer of every member's broker-call verifier.
#[derive(Clone)]
pub struct BrokerCallVerifiers {
    keyed: Hmac<Sha256>,
    digest: Arc<[u8]>,
}

impl std::fmt::Debug for BrokerCallVerifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrokerCallVerifiers(<redacted>)")
    }
}

impl BrokerCallVerifiers {
    #[must_use]
    pub fn from_secret(secret: &str) -> Option<Self> {
        if secret.is_empty() {
            return None;
        }
        let keyed = keyed_hmac(BROKER_CALL_LABEL).ok()?;
        let mut mac = keyed.clone();
        mac.update(secret.as_bytes());
        Some(Self {
            keyed,
            digest: Arc::from(mac.finalize().into_bytes().as_slice()),
        })
    }
}

impl ConferralIssuer<dyn BrokerCallVerifier> for BrokerCallVerifiers {
    fn issue(&self, grant: &MemberGrant) -> Arc<dyn BrokerCallVerifier> {
        Arc::new(MemberBrokerCallVerifier {
            keyed: self.keyed.clone(),
            digest: Arc::clone(&self.digest),
            member: grant.member.clone(),
        })
    }
}

struct MemberBrokerCallVerifier {
    keyed: Hmac<Sha256>,
    digest: Arc<[u8]>,
    member: ExtensionId,
}

impl std::fmt::Debug for MemberBrokerCallVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberBrokerCallVerifier")
            .field("member", &self.member)
            .finish_non_exhaustive()
    }
}

impl BrokerCallVerifier for MemberBrokerCallVerifier {
    fn verify(&self, headers: &axum::http::HeaderMap) -> Result<(), BrokerCallRefused> {
        let presented = headers
            .get(systemprompt_identifiers::headers::CREDENTIAL_BROKER)
            .ok_or(BrokerCallRefused::Absent)?;
        let mut mac = self.keyed.clone();
        mac.update(presented.as_bytes());
        mac.verify_slice(&self.digest)
            .map_err(|_e| BrokerCallRefused::Mismatch)
    }
}
