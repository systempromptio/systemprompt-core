//! Opaque keyset cursor over the `(timestamp, id)` feed ordering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, SecondsFormat, Utc};
use uuid::Uuid;

/// A feed position, encoded as base64url of `<rfc3339 micros>|<uuid>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeysetCursor {
    pub at: DateTime<Utc>,
    pub id: Uuid,
}

/// Why a cursor string did not decode.
#[derive(Debug, thiserror::Error)]
pub enum KeysetCursorError {
    #[error("cursor is not valid base64")]
    Encoding(#[source] base64::DecodeError),
    #[error("cursor is not valid UTF-8")]
    Utf8(#[source] std::string::FromUtf8Error),
    #[error("cursor has no separator")]
    Shape,
    #[error("cursor timestamp is invalid")]
    Timestamp(#[source] chrono::ParseError),
    #[error("cursor id is invalid")]
    Id(#[source] uuid::Error),
}

impl KeysetCursor {
    #[must_use]
    pub const fn new(at: DateTime<Utc>, id: Uuid) -> Self {
        Self { at, id }
    }
}

impl fmt::Display for KeysetCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = format!(
            "{}|{}",
            self.at.to_rfc3339_opts(SecondsFormat::Micros, true),
            self.id
        );
        f.write_str(&URL_SAFE_NO_PAD.encode(raw))
    }
}

impl FromStr for KeysetCursor {
    type Err = KeysetCursorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = URL_SAFE_NO_PAD
            .decode(s)
            .map_err(KeysetCursorError::Encoding)?;
        let raw = String::from_utf8(bytes).map_err(KeysetCursorError::Utf8)?;
        let (at, id) = raw.split_once('|').ok_or(KeysetCursorError::Shape)?;
        let at = DateTime::parse_from_rfc3339(at)
            .map_err(KeysetCursorError::Timestamp)?
            .with_timezone(&Utc);
        let id = Uuid::parse_str(id).map_err(KeysetCursorError::Id)?;
        Ok(Self { at, id })
    }
}

crate::notifications::string_form::string_serde!(KeysetCursor, "KeysetCursor");
