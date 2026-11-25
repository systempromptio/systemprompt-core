//! The core issuers of the member cipher and the member share-token signer,
//! the conferred capabilities that use the deployment's sealed key material
//! without exporting it.
//!
//! [`MemberCiphers`] holds the `encryption_master_key`. Each member's cipher
//! is ChaCha20-Poly1305 (the [`AtRestCipher`]) under
//! HMAC-SHA-256(master key, `systemprompt.member-cipher.v1` NUL member id):
//! a separate key per member and separate from the key core seals with, so a
//! member's handle opens only its own ciphertext.
//!
//! [`MemberShareTokens`] holds the `manifest_signing_secret_seed`. A token is
//! `spst1.<base64url(expiry ‖ payload)>.<base64url(tag)>`, where the tag is
//! HMAC-SHA-256(seed, `systemprompt.member-share-token.v1` NUL member id NUL
//! purpose NUL body): bound to the member, the purpose and the expiry. The
//! token is not a JWT and its key is not the manifest signing key, so no core
//! verifier accepts it, and one member's signer verifies nothing another member
//! signed.
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
    ConferralIssuer, MemberCipher, MemberCipherError, MemberGrant, SHARE_TOKEN_MAX_TTL,
    ShareTokenError, ShareTokenSigner,
};

use crate::at_rest::{AtRestCipher, AtRestCipherError, AtRestHashError, keyed_hmac};

const CIPHER_LABEL: &[u8] = b"systemprompt.member-cipher.v1";
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
    root: Hmac<Sha256>,
}

impl MemberCiphers {
    pub fn from_master_key(master_key: &[u8; 32]) -> Result<Self, AtRestHashError> {
        Ok(Self {
            root: keyed(master_key)?,
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
        let key: [u8; 32] = labelled(&self.root, CIPHER_LABEL, &grant.member)
            .finalize()
            .into_bytes()
            .into();
        Arc::new(MemberCipherHandle(AtRestCipher::from_master_key(key)))
    }
}

#[derive(Debug)]
struct MemberCipherHandle(AtRestCipher);

const fn cipher_error(error: AtRestCipherError) -> MemberCipherError {
    match error {
        AtRestCipherError::Malformed => MemberCipherError::Malformed,
        AtRestCipherError::NotAuthentic
        | AtRestCipherError::KeyUnavailable
        | AtRestCipherError::KeyInvalid => MemberCipherError::NotAuthentic,
    }
}

impl MemberCipher for MemberCipherHandle {
    fn seal(&self, plaintext: &str) -> Result<String, MemberCipherError> {
        self.0.seal(plaintext).map_err(cipher_error)
    }

    fn open(&self, sealed: &str) -> Result<String, MemberCipherError> {
        self.0.open(sealed).map_err(cipher_error)
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
