//! Font-family declarations and self-hosted font-file metadata.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontsConfig {
    pub body: FontConfig,
    pub heading: FontConfig,
    #[serde(default)]
    pub brand: Option<FontConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    pub family: String,
    pub fallback: String,
    #[serde(default)]
    pub files: Vec<FontFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontFile {
    pub path: String,
    pub weight: u16,
    pub style: String,
}
