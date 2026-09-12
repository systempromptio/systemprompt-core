//! Bounded exact file bytes with portable paths and verified content hashes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::invalid;
use super::{ManagedError, Result};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetDigest(String);

impl AssetDigest {
    pub fn of(bytes: &[u8]) -> Self {
        Self(hex::encode(Sha256::digest(bytes)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AssetDigest {
    type Error = ManagedError;

    fn try_from(value: String) -> Result<Self> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("Expected a lowercase SHA-256 digest"));
        }
        Ok(Self(value))
    }
}

impl From<AssetDigest> for String {
    fn from(value: AssetDigest) -> Self {
        value.0
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetFile {
    pub bytes: Vec<u8>,
    pub media_type: String,
    pub executable: bool,
}

impl std::fmt::Debug for AssetFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetFile")
            .field("bytes", &self.bytes.len())
            .field("media_type", &self.media_type)
            .field("executable", &self.executable)
            .finish()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RevisionFiles(pub BTreeMap<String, AssetFile>);

impl RevisionFiles {
    pub fn validate(&self) -> Result<()> {
        if self.0.is_empty() || self.0.len() > 256 {
            return Err(invalid("Expected 1–256 revision files"));
        }
        let mut bytes = 0usize;
        for (path, file) in &self.0 {
            validate_path(path)?;
            bytes = bytes
                .checked_add(file.bytes.len())
                .ok_or_else(|| invalid("File size overflow"))?;
            if bytes > 8 * 1024 * 1024 {
                return Err(invalid("Revision files exceed 8 MiB"));
            }
            if file.media_type.is_empty()
                || file.media_type.len() > 128
                || !file
                    .media_type
                    .bytes()
                    .all(|b| b.is_ascii_graphic() || b == b' ')
            {
                return Err(invalid("Invalid file media type"));
            }
        }
        Ok(())
    }
}

pub(super) fn validate_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 1024
        || path.starts_with('/')
        || path.contains(['\\', ':'])
        || path.chars().any(char::is_control)
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(invalid("File paths must be portable relative paths"));
    }
    Ok(())
}
