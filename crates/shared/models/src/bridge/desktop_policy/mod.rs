//! Validated fleet policy for Claude Desktop gateway deployments.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod validation;
pub use error::DesktopPolicyError;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const SETTINGS_CATALOG: &str = include_str!("../desktop_settings.json");

#[derive(Debug, Clone, Deserialize)]
pub struct DesktopSetting {
    pub key: String,
    pub r#type: String,
    pub min_version: Option<String>,
    pub disposition: String,
    #[serde(default)]
    pub values: Vec<String>,
    pub min: Option<u64>,
    pub max: Option<u64>,
    // JSON: Desktop's audited setting catalog declares heterogeneous defaults.
    pub default: Option<Value>,
    #[serde(default)]
    pub fields: BTreeMap<String, DesktopField>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DesktopField {
    pub r#type: String,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DesktopSettingsCatalog {
    pub schema_version: u32,
    pub settings: Vec<DesktopSetting>,
}

pub fn settings_catalog() -> Result<DesktopSettingsCatalog, serde_json::Error> {
    serde_json::from_str(SETTINGS_CATALOG)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DesktopPolicy {
    pub schema_version: u32,
    // JSON: Managed Desktop settings have catalog-defined, validated value shapes.
    pub settings: BTreeMap<String, Value>,
}

impl<'de> Deserialize<'de> for DesktopPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            #[serde(default = "schema_version")]
            schema_version: u32,
            #[serde(default)]
            // JSON: External managed settings are validated before constructing the policy.
            settings: BTreeMap<String, Value>,
        }
        const fn schema_version() -> u32 {
            1
        }
        let raw = Raw::deserialize(deserializer)?;
        let policy = Self {
            schema_version: raw.schema_version,
            settings: raw.settings,
        };
        policy.validate().map_err(serde::de::Error::custom)?;
        Ok(policy)
    }
}

impl DesktopPolicy {
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }

    pub fn validate(&self) -> Result<(), DesktopPolicyError> {
        validation::validate(self)
    }
}
