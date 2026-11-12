//! The keyset position of a fact listing and its opaque wire token. A token
//! names the reader it was minted for and the last visible fact of the page
//! that reader saw; decoding it for any other reader fails, so a token only
//! ever resumes the listing of the reader who received it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::brain::BrainFact;

/// Where a listing resumes, in its `updated_at DESC, id DESC` order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactCursor {
    pub updated_at: DateTime<Utc>,
    pub id: BrainFactId,
}

/// Why a listing token was refused.
#[derive(Debug, thiserror::Error)]
pub enum CursorError {
    #[error("the cursor is not hex")]
    Hex(#[from] hex::FromHexError),
    #[error("the cursor is malformed")]
    Malformed(#[from] serde_json::Error),
    #[error("the cursor was issued to another reader")]
    Reader,
}

#[derive(Serialize, Deserialize)]
struct Token {
    reader: UserId,
    updated_at: DateTime<Utc>,
    id: BrainFactId,
}

impl FactCursor {
    #[must_use]
    pub fn at(fact: &BrainFact) -> Self {
        Self {
            updated_at: fact.updated_at,
            id: fact.id.clone(),
        }
    }

    #[must_use]
    pub fn encode(&self, reader: &UserId) -> String {
        let token = serde_json::json!({
            "reader": reader,
            "updated_at": self.updated_at,
            "id": self.id,
        });
        hex::encode(token.to_string())
    }

    pub fn decode(token: &str, reader: &UserId) -> Result<Self, CursorError> {
        let token: Token = serde_json::from_slice(&hex::decode(token)?)?;
        if &token.reader != reader {
            return Err(CursorError::Reader);
        }
        Ok(Self {
            updated_at: token.updated_at,
            id: token.id,
        })
    }
}
