//! How `admin secrets reseal` judges one stored value: already under the
//! member's cipher, pre-0.88 ciphertext to re-seal, or undecodable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_database::admin::SealedRow;
use systemprompt_extension::SealedLayout;
use systemprompt_identifiers::ExtensionId;
use systemprompt_security::at_rest::{AtRestCipher, AtRestCipherError, MasterKey, pre_088};
use systemprompt_traits::MEMBER_CIPHER_ENVELOPE;

/// What one row's stored value is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowState {
    MemberSealed,
    Reseal(Vec<u8>),
    Undecodable,
}

/// The two schemes a row is judged under: the pre-0.88 scheme directly under
/// the master key, and the member's at-rest cipher.
pub struct ResealCiphers {
    master_key: MasterKey,
    member: AtRestCipher,
}

impl std::fmt::Debug for ResealCiphers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResealCiphers(<redacted>)")
    }
}

impl ResealCiphers {
    pub fn new(master_key: MasterKey, member: &ExtensionId) -> Result<Self> {
        Ok(Self {
            member: AtRestCipher::for_member(&master_key, member)
                .context("the master key is not a usable HMAC key")?,
            master_key,
        })
    }

    #[must_use]
    pub fn judge(&self, layout: SealedLayout, row: &SealedRow) -> RowState {
        let Ok(stored) = std::str::from_utf8(&row.ciphertext) else {
            return self.raw(layout, row);
        };
        if stored.starts_with(MEMBER_CIPHER_ENVELOPE) {
            return if self.member.open_bytes(stored).is_ok() {
                RowState::MemberSealed
            } else {
                RowState::Undecodable
            };
        }
        self.raw(layout, row)
    }

    fn raw(&self, layout: SealedLayout, row: &SealedRow) -> RowState {
        let opened = match layout {
            SealedLayout::SplitBytea { .. } => {
                pre_088::open(&self.master_key, &row.nonce, &row.ciphertext)
            },
            SealedLayout::HexText => std::str::from_utf8(&row.ciphertext)
                .map_err(|_e| AtRestCipherError::Malformed)
                .and_then(|hex| pre_088::open_hex(&self.master_key, hex)),
        };
        let Ok(plaintext) = opened else {
            return RowState::Undecodable;
        };
        self.member
            .seal_bytes(&plaintext)
            .map_or(RowState::Undecodable, |sealed| {
                RowState::Reseal(sealed.into_bytes())
            })
    }
}
