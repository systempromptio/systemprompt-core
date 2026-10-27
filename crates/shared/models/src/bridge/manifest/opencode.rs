//! The `OpenCode` client settings a marketplace asks the bridge to apply.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::bridge::ids::Sha256Digest;

// Why: a bridge older than the V2 `OpenCode` writer would project a V1 config
// over a V2 client, so a marketplace carrying `opencode` settings requires it.
pub const OPENCODE_V2_MIN_BRIDGE: semver::Version = semver::Version::new(0, 73, 0);

/// Tolerant of unknown keys, unlike the kit-side `OpenCodeMarketplaceConfig`
/// (services manifest), so a bridge ignores a setting a newer gateway adds.
///
/// `themes` name the theme files the plugin bundles ship under
/// `hosts/opencode/themes/<name>.json`, each pinned by digest, so a user can
/// switch between them; `default_theme` (with `theme_mode`) is the one the
/// bridge selects in `cli.json` where the user has not chosen one.
/// `cli_defaults` are top-level `opencode.json` keys the bridge sets only where
/// the user has not set them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "ManifestOpenCodeWire")]
pub struct ManifestOpenCode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_client_version: Option<semver::Version>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<ThemeRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_mode: Option<ThemeMode>,
    #[serde(skip_serializing_if = "serde_json::Map::is_empty")]
    // JSON: client defaults are foreign `opencode.json` keys, passed through
    pub cli_defaults: serde_json::Map<String, serde_json::Value>,
}

impl ManifestOpenCode {
    /// The default theme's ref, when it names one of [`Self::themes`].
    #[must_use]
    pub fn default_theme_ref(&self) -> Option<&ThemeRef> {
        let name = self.default_theme.as_deref()?;
        self.themes.iter().find(|t| t.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeRef {
    pub name: String,
    pub sha: Sha256Digest,
}

/// `OpenCode`'s `cli.json` `theme.mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    System,
    Dark,
    Light,
}

// Why: a 0.73 gateway sends one `theme`, which a newer bridge still reads as
// the only theme and the default.
#[derive(Deserialize)]
struct ManifestOpenCodeWire {
    #[serde(default)]
    min_client_version: Option<semver::Version>,
    #[serde(default)]
    themes: Vec<ThemeRef>,
    #[serde(default)]
    theme: Option<ThemeRef>,
    #[serde(default)]
    default_theme: Option<String>,
    #[serde(default)]
    theme_mode: Option<ThemeMode>,
    #[serde(default)]
    cli_defaults: serde_json::Map<String, serde_json::Value>,
}

impl From<ManifestOpenCodeWire> for ManifestOpenCode {
    fn from(wire: ManifestOpenCodeWire) -> Self {
        let mut themes = wire.themes;
        let mut default_theme = wire.default_theme;
        if let Some(single) = wire.theme {
            default_theme.get_or_insert_with(|| single.name.clone());
            if !themes.iter().any(|t| t.name == single.name) {
                themes.push(single);
            }
        }
        Self {
            min_client_version: wire.min_client_version,
            themes,
            default_theme,
            theme_mode: wire.theme_mode,
            cli_defaults: wire.cli_defaults,
        }
    }
}
