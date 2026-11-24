//! Opaque keyset cursor over a `(timestamp, id)` listing order, and one page
//! of a keyset listing.
//!
//! A listing ordered `created_at DESC, id DESC` resumes strictly after the
//! last row it handed out, so rows inserted between two page reads never
//! shift or repeat a page the way an `OFFSET` does. The cursor carries the
//! last row's ordering key and nothing else; on the wire it is base64url of
//! `<rfc3339 micros>|<id>`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, SecondsFormat, Utc};
use uuid::Uuid;

/// A listing position: the ordering key of the last row a page handed out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeysetCursor<K = Uuid> {
    pub at: DateTime<Utc>,
    pub id: K,
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
    Id(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl<K> KeysetCursor<K> {
    #[must_use]
    pub const fn new(at: DateTime<Utc>, id: K) -> Self {
        Self { at, id }
    }
}

impl<K: fmt::Display> fmt::Display for KeysetCursor<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = format!(
            "{}|{}",
            self.at.to_rfc3339_opts(SecondsFormat::Micros, true),
            self.id
        );
        f.write_str(&URL_SAFE_NO_PAD.encode(raw))
    }
}

impl<K> FromStr for KeysetCursor<K>
where
    K: FromStr,
    K::Err: std::error::Error + Send + Sync + 'static,
{
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
        let id = id
            .parse::<K>()
            .map_err(|source| KeysetCursorError::Id(Box::new(source)))?;
        Ok(Self { at, id })
    }
}

impl<K: fmt::Display> serde::Serialize for KeysetCursor<K> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, K> serde::Deserialize<'de> for KeysetCursor<K>
where
    K: FromStr,
    K::Err: std::error::Error + Send + Sync + 'static,
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

impl<K> schemars::JsonSchema for KeysetCursor<K> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "KeysetCursor".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

/// One page of a keyset listing: its rows and, when more follow, the cursor
/// after the last of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeysetPage<T, K = Uuid> {
    pub items: Vec<T>,
    pub next_cursor: Option<KeysetCursor<K>>,
}

impl<T, K> KeysetPage<T, K> {
    #[must_use]
    pub fn from_lookahead(
        mut items: Vec<T>,
        limit: usize,
        key: impl Fn(&T) -> KeysetCursor<K>,
    ) -> Self {
        if items.len() <= limit {
            return Self {
                items,
                next_cursor: None,
            };
        }
        items.truncate(limit);
        let next_cursor = items.last().map(key);
        Self { items, next_cursor }
    }
}
