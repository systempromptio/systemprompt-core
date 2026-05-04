//! Font-family declarations and self-hosted font-file metadata.

use serde::{Deserialize, Serialize};

/// Font-family declarations (body, heading, brand).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontsConfig {
    /// Body-text font.
    pub body: FontConfig,
    /// Heading font.
    pub heading: FontConfig,
    /// Optional brand / display font.
    #[serde(default)]
    pub brand: Option<FontConfig>,
}

/// One font-family declaration with optional bundled font files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    /// Primary CSS font-family name.
    pub family: String,
    /// CSS fallback stack appended after [`Self::family`].
    pub fallback: String,
    /// Self-hosted font files associated with this family.
    #[serde(default)]
    pub files: Vec<FontFile>,
}

/// One self-hosted font file in [`FontConfig::files`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontFile {
    /// Path to the font file.
    pub path: String,
    /// CSS `font-weight` numeric value.
    pub weight: u16,
    /// CSS `font-style` value (`normal`, `italic`, ...).
    pub style: String,
}
