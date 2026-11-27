//! The device's reading of the signed manifest.
//!
//! The body is `systemprompt-models`'
//! [`SignedManifest`](systemprompt_models::bridge::manifest::SignedManifest),
//! the type the instance signs. `Manifest` holds it beside the one block that
//! type does not carry: a marketplace's `harness` settings (`HostSettings`),
//! read from the same verified payload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Deref;

use serde::{Deserialize, Serialize};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::ids::HostExtensionId;
use systemprompt_models::bridge::manifest::{PluginEntry, SignedManifest, ThemeMode, ThemeRef};

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[must_use]
pub fn probe_schema_version(payload: &str) -> Option<u32> {
    #[derive(Deserialize)]
    struct Probe {
        #[serde(default)]
        min_schema_version: Option<u32>,
    }
    serde_json::from_str::<Probe>(payload)
        .ok()?
        .min_schema_version
}

/// A verified manifest body and the host settings its marketplaces carry, in
/// marketplace order.
#[derive(Debug, Clone)]
pub struct Manifest {
    signed: SignedManifest,
    host_settings: Vec<HostSettings>,
}

impl Manifest {
    pub fn parse(payload: &str) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct Settings {
            #[serde(default)]
            marketplaces: Vec<MarketplaceSettings>,
        }
        #[derive(Deserialize)]
        struct MarketplaceSettings {
            #[serde(default)]
            harness: Option<HostSettings>,
        }
        let signed: SignedManifest = serde_json::from_str(payload)?;
        let settings: Settings = serde_json::from_str(payload)?;
        Ok(Self {
            signed,
            host_settings: settings
                .marketplaces
                .into_iter()
                .filter_map(|marketplace| marketplace.harness)
                .collect(),
        })
    }

    #[must_use]
    pub const fn signed(&self) -> &SignedManifest {
        &self.signed
    }

    #[must_use]
    pub const fn schema_too_new(&self) -> bool {
        self.signed.min_schema_version > SUPPORTED_SCHEMA_VERSION
    }

    #[must_use]
    pub fn governance_plugin(&self) -> Option<&PluginEntry> {
        self.signed
            .plugins
            .iter()
            .find(|plugin| plugin.hooks.governance)
    }

    #[must_use]
    pub fn model_families(&self, host: HostKind) -> Option<&[String]> {
        self.signed
            .host_model_protocols
            .get(&host)
            .map(Vec::as_slice)
    }

    #[must_use]
    pub fn extensions_for(&self, host: HostKind) -> Option<&[HostExtensionId]> {
        self.signed.host_extensions.get(&host).map(Vec::as_slice)
    }

    #[must_use]
    pub fn host_settings(&self) -> Option<&HostSettings> {
        self.host_settings.first()
    }

    #[must_use]
    pub fn marketplace_settings(&self) -> &[HostSettings] {
        &self.host_settings
    }
}

impl Deref for Manifest {
    type Target = SignedManifest;

    fn deref(&self) -> &SignedManifest {
        &self.signed
    }
}

impl From<SignedManifest> for Manifest {
    fn from(signed: SignedManifest) -> Self {
        Self {
            signed,
            host_settings: Vec::new(),
        }
    }
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
