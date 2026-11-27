//! A marketplace of the manifest and the harness settings block it may carry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::Sha256Digest;
use crate::ids::{MarketplaceId, PluginId};

/// An enabled marketplace and the manifest plugins it carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marketplace {
    pub id: MarketplaceId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub plugin_ids: Vec<PluginId>,
    #[serde(default)]
    pub harness: Option<HostSettings>,
}

/// A theme file a plugin bundle ships under `themes/<name>.json`, pinned by
/// digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeRef {
    pub name: String,
    pub sha: Sha256Digest,
}

/// How the default theme is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    System,
    Dark,
    Light,
}

/// A marketplace's settings for the harness: client defaults set only where the
/// user has not chosen, the themes the bundles ship, and the default theme.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSettings {
    // JSON: client defaults are free-form settings keys the harness applies where the user has not
    // set them.
    #[serde(default)]
    pub defaults: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub themes: Vec<ThemeRef>,
    #[serde(default)]
    pub default_theme: Option<String>,
    #[serde(default)]
    pub theme_mode: Option<ThemeMode>,
}

impl HostSettings {
    #[must_use]
    pub fn default_theme_ref(&self) -> Option<&ThemeRef> {
        let name = self.default_theme.as_deref()?;
        self.themes.iter().find(|theme| theme.name == name)
    }

    #[must_use]
    pub fn default_model(&self) -> Option<(&str, &str)> {
        let model = self.defaults.get("model")?.as_str()?;
        let (provider, id) = model.split_once('/')?;
        (!provider.is_empty() && !id.is_empty()).then_some((provider, id))
    }
}
