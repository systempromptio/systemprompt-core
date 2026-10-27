//! Opaque keyset cursor over `(timestamp, id)` orderings.

use std::fmt;
use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListCursor {
    pub updated_at: DateTime<Utc>,
    pub id: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum ListCursorError {
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

impl ListCursor {
    #[must_use]
    pub const fn new(updated_at: DateTime<Utc>, id: Uuid) -> Self {
        Self { updated_at, id }
    }
}

impl fmt::Display for ListCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = format!("{}|{}", self.updated_at.to_rfc3339(), self.id);
        f.write_str(&URL_SAFE_NO_PAD.encode(raw))
    }
}

impl FromStr for ListCursor {
    type Err = ListCursorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = URL_SAFE_NO_PAD
            .decode(s)
            .map_err(ListCursorError::Encoding)?;
        let raw = String::from_utf8(bytes).map_err(ListCursorError::Utf8)?;
        let (at, id) = raw.split_once('|').ok_or(ListCursorError::Shape)?;
        let updated_at = DateTime::parse_from_rfc3339(at)
            .map_err(ListCursorError::Timestamp)?
            .with_timezone(&Utc);
        let id = Uuid::parse_str(id).map_err(ListCursorError::Id)?;
        Ok(Self { updated_at, id })
    }
}
