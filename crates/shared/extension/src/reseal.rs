//! Columns a member declares as holding ciphertext sealed under the raw
//! deployment master key, which core re-seals under that member's cipher.
//!
//! An install member that sealed values with the `encryption_master_key`
//! before it read the conferred member cipher declares each such column in
//! `contribute`:
//!
//! ```ignore
//! c.reseal_column(ResealColumn {
//!     table: "mcp_connector_credentials",
//!     keys: &["user_id", "provider"],
//!     ciphertext: "ciphertext",
//!     layout: SealedLayout::SplitBytea { nonce: "nonce" },
//! });
//! ```
//!
//! The raw scheme is ChaCha20-Poly1305 directly under the master key, the one
//! releases before 0.88 sealed with, core included. [`SealedLayout`] says how
//! a column stores it: the 12-byte nonce in its own `BYTEA` column beside a
//! `BYTEA` ciphertext, or the hex nonce and ciphertext in one `TEXT` column
//! (core's `AtRestCipher` shape). Core members declare their own at-rest
//! columns the same way. `systemprompt admin secrets reseal --member <id>`
//! opens each value under the raw scheme and writes the member's `spmc1.`
//! envelope in its place. The declaration is
//! a claim, not a grant: [`AdmittedReseals`] admits it only over a table the
//! declaring member's own schema SQL creates and no other admitted member's
//! creates, so a member cannot name a core table or another member's table.
//! Every identifier is a bare lowercase SQL identifier, quoted by the
//! database layer when the command runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ExtensionId;
use thiserror::Error;

use crate::admitted::BuiltState;
use crate::admitted_purge::{PurgeAdmissionError, own_tables};
use crate::metadata::SchemaDefinition;

/// One column of raw-master-key ciphertext a member owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResealColumn {
    pub table: &'static str,
    pub keys: &'static [&'static str],
    pub ciphertext: &'static str,
    pub layout: SealedLayout,
}

/// How a declared column stores raw-scheme ciphertext.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealedLayout {
    SplitBytea { nonce: &'static str },
    HexText,
}

impl ResealColumn {
    #[must_use]
    pub const fn nonce(&self) -> Option<&'static str> {
        match self.layout {
            SealedLayout::SplitBytea { nonce } => Some(nonce),
            SealedLayout::HexText => None,
        }
    }
}

/// Why a member's re-seal declarations were refused at admission.
#[derive(Debug, Error)]
pub enum ResealAdmissionError {
    #[error(
        "member '{member}' declares a re-seal over table '{table}', which is not among its own \
         schema tables"
    )]
    ForeignTable { member: ExtensionId, table: String },
    #[error(
        "member '{member}' declares a re-seal over table '{table}', which member '{other}' also \
         creates"
    )]
    SharedTable {
        member: ExtensionId,
        other: ExtensionId,
        table: String,
    },
    #[error("member '{member}' declares a re-seal over table '{table}' with an invalid column set")]
    Columns { member: ExtensionId, table: String },
    #[error(
        "member '{member}' declares a re-seal identifier '{identifier}' that is not a bare SQL identifier"
    )]
    Identifier {
        member: ExtensionId,
        identifier: &'static str,
    },
    #[error(transparent)]
    Schema(#[from] PurgeAdmissionError),
}

/// One admitted re-seal declaration with its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedReseal {
    pub owner: ExtensionId,
    pub column: ResealColumn,
}

/// The owner-stamped re-seal declarations of an admitted composition.
#[derive(Debug, Clone, Default)]
pub struct AdmittedReseals {
    entries: Vec<OwnedReseal>,
}

/// One member's re-seal declarations with what admission judges them by.
#[derive(Debug, Clone, Copy)]
pub struct ResealClaim<'a> {
    pub owner: &'a ExtensionId,
    pub schemas: &'a [SchemaDefinition],
    pub columns: &'a [ResealColumn],
}

fn bare(identifier: &str) -> bool {
    let mut chars = identifier.chars();
    identifier.len() <= 63
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn check_columns(owner: &ExtensionId, column: &ResealColumn) -> Result<(), ResealAdmissionError> {
    let named = std::iter::once(column.table)
        .chain(column.keys.iter().copied())
        .chain(std::iter::once(column.ciphertext))
        .chain(column.nonce());
    for identifier in named {
        if !bare(identifier) {
            return Err(ResealAdmissionError::Identifier {
                member: owner.clone(),
                identifier,
            });
        }
    }
    let mut columns: Vec<&str> = column.keys.to_vec();
    columns.push(column.ciphertext);
    columns.extend(column.nonce());
    let count = columns.len();
    columns.sort_unstable();
    columns.dedup();
    if column.keys.is_empty() || columns.len() != count {
        return Err(ResealAdmissionError::Columns {
            member: owner.clone(),
            table: column.table.to_owned(),
        });
    }
    Ok(())
}

impl AdmittedReseals {
    pub fn of(built: &BuiltState) -> Result<Self, ResealAdmissionError> {
        let claims: Vec<ResealClaim<'_>> = built
            .members()
            .map(|member| ResealClaim {
                owner: &member.owner,
                schemas: member.sink.schemas(),
                columns: member.sink.reseal_columns(),
            })
            .collect();
        Self::admit(&claims)
    }

    pub fn admit(claims: &[ResealClaim<'_>]) -> Result<Self, ResealAdmissionError> {
        let owned = claims
            .iter()
            .map(|claim| own_tables(claim.owner, claim.schemas))
            .collect::<Result<Vec<_>, _>>()?;
        let mut entries = Vec::new();
        for (index, claim) in claims.iter().enumerate() {
            for column in claim.columns {
                check_columns(claim.owner, column)?;
                if !owned[index].contains(column.table) {
                    return Err(ResealAdmissionError::ForeignTable {
                        member: claim.owner.clone(),
                        table: column.table.to_owned(),
                    });
                }
                if let Some((other, _)) = claims
                    .iter()
                    .zip(&owned)
                    .enumerate()
                    .find(|(at, (_, tables))| *at != index && tables.contains(column.table))
                    .map(|(_, pair)| pair)
                {
                    return Err(ResealAdmissionError::SharedTable {
                        member: claim.owner.clone(),
                        other: other.owner.clone(),
                        table: column.table.to_owned(),
                    });
                }
                entries.push(OwnedReseal {
                    owner: claim.owner.clone(),
                    column: *column,
                });
            }
        }
        Ok(Self { entries })
    }

    #[must_use]
    pub fn of_member(&self, member: &ExtensionId) -> Vec<&ResealColumn> {
        self.entries
            .iter()
            .filter(|entry| &entry.owner == member)
            .map(|entry| &entry.column)
            .collect()
    }

    #[must_use]
    pub fn entries(&self) -> &[OwnedReseal] {
        &self.entries
    }

    #[must_use]
    pub fn members(&self) -> Vec<&ExtensionId> {
        let mut members: Vec<&ExtensionId> = Vec::new();
        for entry in &self.entries {
            if !members.contains(&&entry.owner) {
                members.push(&entry.owner);
            }
        }
        members
    }
}
