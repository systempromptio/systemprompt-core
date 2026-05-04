//! [`IncludableString`] — a string field that can either carry inline
//! content or a `!include <path>` reference resolved at load time.

use serde::{Deserialize, Deserializer, Serialize};

/// A string field that may either be supplied inline or as a reference
/// to a sibling file via `!include <path>`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum IncludableString {
    /// Inline literal content.
    Inline(String),
    /// File include reference.
    Include {
        /// Path of the file to include, relative to the document.
        path: String,
    },
}

impl<'de> Deserialize<'de> for IncludableString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(s.strip_prefix("!include ").map_or_else(
            || Self::Inline(s.clone()),
            |path| Self::Include {
                path: path.trim().to_string(),
            },
        ))
    }
}

impl IncludableString {
    /// True if this value carries an `!include` reference.
    #[must_use]
    pub const fn is_include(&self) -> bool {
        matches!(self, Self::Include { .. })
    }

    /// Borrow the inline string, if any.
    #[must_use]
    pub fn as_inline(&self) -> Option<&str> {
        match self {
            Self::Inline(s) => Some(s),
            Self::Include { .. } => None,
        }
    }
}

impl Default for IncludableString {
    fn default() -> Self {
        Self::Inline(String::new())
    }
}
