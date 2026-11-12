//! Two-phase rotation of the RS256 token key.
//!
//! [`stage`] generates the next key pair and records it as staged: every
//! verifier that boots with it accepts tokens it signs, and the JWKS publishes
//! it, but nothing signs with it yet. Once every replica has restarted with the
//! staged key, [`promote`] makes it the signer and moves the outgoing key into
//! the retired list. Because the new signer is already in every replica's
//! verification set, no replica ever meets a token signed by a key it does not
//! know. A single-replica deployment may run both steps back to back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration, Utc};
use systemprompt_config::{PreviousSigningKey, SigningKeyState};

use crate::keys::RsaSigningKey;
use crate::keys::signing_keys::{SigningKeysError, SigningKeysResult};

/// How a promotion treats the outgoing key and the keys already retired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationMode {
    Overlap(Duration),
    Revoke,
}

pub fn stage(state: &SigningKeyState) -> SigningKeysResult<(RsaSigningKey, SigningKeyState)> {
    if let Some(pem) = &state.staged_private_pem {
        let staged = RsaSigningKey::from_pkcs8_pem(pem)?;
        return Err(SigningKeysError::AlreadyStaged {
            kid: staged.kid().to_owned(),
        });
    }
    let staged = RsaSigningKey::generate()?;
    let next = SigningKeyState {
        active_private_pem: state.active_private_pem.clone(),
        staged_private_pem: Some(staged.to_pkcs8_pem()?),
        previous: state.previous.clone(),
    };
    Ok((staged, next))
}

pub fn promote(
    state: &SigningKeyState,
    mode: RotationMode,
    now: DateTime<Utc>,
) -> SigningKeysResult<(RsaSigningKey, SigningKeyState)> {
    let staged_pem = state
        .staged_private_pem
        .as_ref()
        .ok_or(SigningKeysError::NothingStaged)?;
    let promoted = RsaSigningKey::from_pkcs8_pem(staged_pem)?;
    let outgoing = RsaSigningKey::from_pkcs8_pem(&state.active_private_pem)?;
    let previous = match mode {
        RotationMode::Revoke => Vec::new(),
        RotationMode::Overlap(overlap) => {
            let mut kept: Vec<PreviousSigningKey> = state
                .previous
                .iter()
                .filter(|entry| {
                    now < entry.expires_at
                        && entry.kid != outgoing.kid()
                        && entry.kid != promoted.kid()
                })
                .cloned()
                .collect();
            kept.push(PreviousSigningKey {
                kid: outgoing.kid().to_owned(),
                public_pem: outgoing.public_key_pem()?,
                expires_at: now + overlap,
            });
            kept
        },
    };
    let next = SigningKeyState {
        active_private_pem: staged_pem.clone(),
        staged_private_pem: None,
        previous,
    };
    Ok((promoted, next))
}
