//! `OpenCode` client settings a marketplace declares for its subscribers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::manifest::ThemeMode;
use systemprompt_models::bridge::opencode::MIN_CLIENT_VERSION;
use systemprompt_models::errors::ServicesValidationError;

/// `OpenCode` settings the bridge applies for a marketplace's subscribers.
///
/// Each of `themes` names `hosts/opencode/themes/<name>.json` in one of the
/// marketplace's plugin bundles and pins its digest; `default_theme` (with
/// `theme_mode`) is selected in the user's `cli.json` unless they chose one.
/// The single-theme `theme:` form is still read as the only theme and the
/// default. `cli_defaults` are top-level `opencode.json` keys set only where
/// the user has not set them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "OpenCodeMarketplaceWire")]
pub struct OpenCodeMarketplaceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_client_version: Option<semver::Version>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<OpenCodeThemeConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_mode: Option<ThemeMode>,
    #[serde(skip_serializing_if = "serde_json::Map::is_empty")]
    // JSON: client defaults are foreign `opencode.json` keys, passed through
    pub cli_defaults: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenCodeThemeConfig {
    pub name: String,
    pub sha256: Sha256Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenCodeMarketplaceWire {
    #[serde(default)]
    min_client_version: Option<semver::Version>,
    #[serde(default)]
    theme: Option<OpenCodeThemeConfig>,
    #[serde(default)]
    themes: Vec<OpenCodeThemeConfig>,
    #[serde(default)]
    default_theme: Option<String>,
    #[serde(default)]
    theme_mode: Option<ThemeMode>,
    #[serde(default)]
    cli_defaults: serde_json::Map<String, serde_json::Value>,
}

impl TryFrom<OpenCodeMarketplaceWire> for OpenCodeMarketplaceConfig {
    type Error = &'static str;

    fn try_from(wire: OpenCodeMarketplaceWire) -> Result<Self, Self::Error> {
        let (themes, default_theme) = match (wire.theme, wire.themes) {
            (Some(_), themes) if !themes.is_empty() => {
                return Err("set either `theme` or `themes`, not both");
            },
            (Some(single), _) => {
                let default = wire.default_theme.unwrap_or_else(|| single.name.clone());
                (vec![single], Some(default))
            },
            (None, themes) => (themes, wire.default_theme),
        };
        Ok(Self {
            min_client_version: wire.min_client_version,
            themes,
            default_theme,
            theme_mode: wire.theme_mode,
            cli_defaults: wire.cli_defaults,
        })
    }
}

// Why: `theme` moved to `OpenCode` V2's `cli.json`; an `opencode.json` key the
// V2 schema does not declare is dropped silently.
const RESERVED_DEFAULTS: &[&str] = &["$schema", "providers", "policies", "mcp", "model", "theme"];

fn is_file_stem(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl OpenCodeMarketplaceConfig {
    pub(super) fn validate(&self, key: &str) -> Result<(), ServicesValidationError> {
        let invalid =
            |what: String| ServicesValidationError::invalid_field(format!("Marketplace '{key}': {what}"));
        if let Some(version) = &self.min_client_version
            && *version < MIN_CLIENT_VERSION
        {
            return Err(invalid(format!(
                "opencode.min_client_version must be at least {MIN_CLIENT_VERSION}"
            )));
        }
        let mut names = BTreeSet::new();
        for theme in &self.themes {
            if !is_file_stem(&theme.name) {
                return Err(invalid(format!(
                    "opencode theme '{}' must be a plain file stem",
                    theme.name
                )));
            }
            if !names.insert(theme.name.as_str()) {
                return Err(invalid(format!(
                    "opencode theme '{}' is declared twice",
                    theme.name
                )));
            }
        }
        if let Some(default) = &self.default_theme
            && !names.contains(default.as_str())
        {
            return Err(invalid(format!(
                "opencode.default_theme '{default}' is not one of opencode.themes"
            )));
        }
        if let Some(reserved) = self
            .cli_defaults
            .keys()
            .find(|k| RESERVED_DEFAULTS.contains(&k.as_str()))
        {
            return Err(invalid(format!(
                "opencode.cli_defaults may not set '{reserved}'"
            )));
        }
        Ok(())
    }
}
