use std::cmp::Ordering;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const MIN_HEX_SUFFIX_LEN: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum ManifestVersionParseError {
    #[error("manifest version missing '-' separator: {0}")]
    NoSeparator(String),
    #[error("manifest version timestamp not RFC3339: {input}: {source}")]
    BadTimestamp {
        input: String,
        #[source]
        source: chrono::ParseError,
    },
    #[error("manifest version suffix must be hex with at least {MIN_HEX_SUFFIX_LEN} chars: {0}")]
    BadSuffix(String),
}

#[derive(Debug, Clone)]
struct Parsed {
    timestamp: DateTime<Utc>,
    suffix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ManifestVersion {
    raw: String,
    parsed: Parsed,
}

impl ManifestVersion {
    pub fn try_new(s: impl Into<String>) -> Result<Self, ManifestVersionParseError> {
        let raw = s.into();
        let (prefix, suffix) = raw
            .rsplit_once('-')
            .ok_or_else(|| ManifestVersionParseError::NoSeparator(raw.clone()))?;
        let timestamp = DateTime::parse_from_rfc3339(prefix)
            .map_err(|source| ManifestVersionParseError::BadTimestamp {
                input: raw.clone(),
                source,
            })?
            .with_timezone(&Utc);
        if suffix.len() < MIN_HEX_SUFFIX_LEN || !suffix.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ManifestVersionParseError::BadSuffix(raw));
        }
        let parsed = Parsed {
            timestamp,
            suffix: suffix.to_string(),
        };
        Ok(Self { raw, parsed })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

impl fmt::Display for ManifestVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl PartialEq for ManifestVersion {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for ManifestVersion {}

impl std::hash::Hash for ManifestVersion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl PartialOrd for ManifestVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ManifestVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.parsed
            .timestamp
            .cmp(&other.parsed.timestamp)
            .then_with(|| self.parsed.suffix.cmp(&other.parsed.suffix))
    }
}

impl TryFrom<String> for ManifestVersion {
    type Error = ManifestVersionParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<ManifestVersion> for String {
    fn from(value: ManifestVersion) -> Self {
        value.raw
    }
}
