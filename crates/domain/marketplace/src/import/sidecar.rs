//! The two `systemprompt.yaml` sidecars an Anthropic tree may carry.
//!
//! A sidecar holds *only* what Anthropic's format has no slot for — including
//! `title`, the human display name, because Anthropic's `name` is the id.
//! Anything derivable from `marketplace.json` or `plugin.json` is forbidden
//! here, so the importer never has two sources for one fact: the sidecar and
//! the derived fields are a disjoint union, never a merge. [`FORBIDDEN_KEYS`]
//! is that rejected set, checked before deserialisation so the error can name
//! the key and say where the value actually comes from; `deny_unknown_fields`
//! then rejects everything else unrecognised.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde::Deserialize;
use systemprompt_models::services::marketplace::{MarketplaceAccess, MarketplaceVisibility};
use systemprompt_models::services::plugin::{PluginComponentRef, PluginHooksRef, PluginScript};

use crate::error::MarketplaceError;

pub const SIDECAR_RELPATH: &str = ".claude-plugin/systemprompt.yaml";

pub const SIDECAR_SCHEMA_VERSION: u32 = 1;

pub const FORBIDDEN_KEYS: &[&str] = &[
    "id",
    "name",
    "description",
    "version",
    "author",
    "keywords",
    "license",
    "plugins",
    "skills",
];

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketplaceSidecar {
    pub schema: u32,
    #[serde(default)]
    pub marketplace: MarketplaceSidecarBody,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketplaceSidecarBody {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub visibility: MarketplaceVisibility,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub access: MarketplaceAccess,
    #[serde(default)]
    pub mcp_servers: PluginComponentRef,
    #[serde(default)]
    pub agents: PluginComponentRef,
    #[serde(default)]
    pub artifacts: PluginComponentRef,
}

impl Default for MarketplaceSidecar {
    fn default() -> Self {
        Self {
            schema: SIDECAR_SCHEMA_VERSION,
            marketplace: MarketplaceSidecarBody::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSidecar {
    pub schema: u32,
    #[serde(default)]
    pub plugin: PluginSidecarBody,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSidecarBody {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub mcp_servers: PluginComponentRef,
    #[serde(default)]
    pub agents: PluginComponentRef,
    #[serde(default)]
    pub artifacts: PluginComponentRef,
    #[serde(default)]
    pub content_sources: PluginComponentRef,
    #[serde(default)]
    pub rules: PluginComponentRef,
    #[serde(default)]
    pub hooks: PluginHooksRef,
    #[serde(default)]
    pub scripts: Vec<PluginScript>,
}

impl Default for PluginSidecar {
    fn default() -> Self {
        Self {
            schema: SIDECAR_SCHEMA_VERSION,
            plugin: PluginSidecarBody::default(),
        }
    }
}

impl Default for MarketplaceSidecarBody {
    fn default() -> Self {
        Self {
            title: None,
            visibility: MarketplaceVisibility::default(),
            enabled: true,
            access: MarketplaceAccess::default(),
            mcp_servers: PluginComponentRef::default(),
            agents: PluginComponentRef::default(),
            artifacts: PluginComponentRef::default(),
        }
    }
}

impl Default for PluginSidecarBody {
    fn default() -> Self {
        Self {
            title: None,
            category: None,
            enabled: true,
            mcp_servers: PluginComponentRef::default(),
            agents: PluginComponentRef::default(),
            artifacts: PluginComponentRef::default(),
            content_sources: PluginComponentRef::default(),
            rules: PluginComponentRef::default(),
            hooks: PluginHooksRef::default(),
            scripts: Vec::new(),
        }
    }
}

pub fn load_marketplace_sidecar(path: &Path) -> Result<MarketplaceSidecar, MarketplaceError> {
    load_sidecar(path, "marketplace")
}

pub fn load_plugin_sidecar(path: &Path) -> Result<PluginSidecar, MarketplaceError> {
    load_sidecar(path, "plugin")
}

fn load_sidecar<T>(path: &Path, section: &str) -> Result<T, MarketplaceError>
where
    T: Default + for<'de> Deserialize<'de>,
{
    if !path.is_file() {
        return Ok(T::default());
    }
    let text = std::fs::read_to_string(path).map_err(|e| err(path, e.to_string()))?;
    let raw: serde_yaml::Value =
        serde_yaml::from_str(&text).map_err(|e| err(path, e.to_string()))?;
    reject_forbidden_keys(path, &raw, section)?;
    check_schema(path, &raw)?;
    serde_yaml::from_value(raw).map_err(|e| err(path, e.to_string()))
}

fn check_schema(path: &Path, raw: &serde_yaml::Value) -> Result<(), MarketplaceError> {
    let schema = raw.get("schema").and_then(serde_yaml::Value::as_u64);
    match schema {
        Some(v) if v == u64::from(SIDECAR_SCHEMA_VERSION) => Ok(()),
        Some(v) => Err(err(
            path,
            format!("sidecar schema {v} is not supported (expected {SIDECAR_SCHEMA_VERSION})"),
        )),
        None => Err(err(
            path,
            format!("sidecar is missing 'schema: {SIDECAR_SCHEMA_VERSION}'"),
        )),
    }
}

fn reject_forbidden_keys(
    path: &Path,
    raw: &serde_yaml::Value,
    section: &str,
) -> Result<(), MarketplaceError> {
    let Some(body) = raw.get(section).and_then(serde_yaml::Value::as_mapping) else {
        return Ok(());
    };
    for key in body.keys() {
        let Some(key) = key.as_str() else { continue };
        if FORBIDDEN_KEYS.contains(&key) {
            return Err(err(
                path,
                format!(
                    "'{section}.{key}' is not allowed in a sidecar — it is derived from the \
                     Anthropic manifest and would become a second source of truth"
                ),
            ));
        }
    }
    Ok(())
}

fn err(path: &Path, message: String) -> MarketplaceError {
    MarketplaceError::Import {
        path: path.display().to_string(),
        message,
    }
}
