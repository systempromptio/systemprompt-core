//! Authoring locations and exact captures are different concepts.

use serde::{Deserialize, Serialize};

use super::assets::validate_path;
use super::error::invalid;
use super::{AssetDigest, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceSpec {
    Git {
        repository: String,
        reference: String,
        subdirectory: Option<String>,
        credential_reference: Option<String>,
    },
    LocalTree {
        root: String,
    },
    Managed,
}

impl SourceSpec {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Git { .. } => "git",
            Self::LocalTree { .. } => "local_tree",
            Self::Managed => "managed",
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Git {
                repository,
                reference,
                subdirectory,
                credential_reference,
            } => {
                // Transport credentials belong in the secret store, not the URL.
                if !repository.starts_with("https://")
                    || repository.contains(['@', '?', '#'])
                    || repository.len() > 2048
                    || repository.chars().any(char::is_whitespace)
                    || repository.len() <= 8
                {
                    return Err(invalid(
                        "Git sources require a credential-free HTTPS repository",
                    ));
                }
                if reference.is_empty()
                    || reference.len() > 200
                    || reference.starts_with('-')
                    || reference.chars().any(char::is_whitespace)
                {
                    return Err(invalid("Invalid Git reference"));
                }
                if let Some(path) = subdirectory {
                    validate_path(path)?;
                }
                if let Some(key) = credential_reference {
                    validate_key(key)?;
                }
            },
            Self::LocalTree { root } => {
                if root.is_empty() || root.len() > 4096 || root.chars().any(char::is_control) {
                    return Err(invalid("Invalid local source root"));
                }
            },
            Self::Managed => {},
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotProvenance {
    pub source_kind: String,
    pub commit: Option<String>,
    pub tree_digest: AssetDigest,
    pub importer_version: String,
}

impl SnapshotProvenance {
    pub fn validate(&self, source: &SourceSpec) -> Result<()> {
        if self.source_kind != source.kind()
            || self.importer_version.is_empty()
            || self.importer_version.len() > 128
        {
            return Err(invalid("Snapshot must identify its source and importer"));
        }
        match (&self.commit, source) {
            (Some(commit), SourceSpec::Git { .. })
                if matches!(commit.len(), 40 | 64)
                    && commit
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) => {},
            (None, SourceSpec::LocalTree { .. } | SourceSpec::Managed) => {},
            _ => {
                return Err(invalid(
                    "Git snapshots require an exact commit; other sources omit it",
                ));
            },
        }
        Ok(())
    }
}

pub(super) fn validate_key(key: &str) -> Result<()> {
    if key.is_empty()
        || key.len() > 200
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/'))
    {
        return Err(invalid("Invalid resource key"));
    }
    Ok(())
}
