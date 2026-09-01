//! The bridge-owned local marketplace and plugin descriptors Codex reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::host_sync::ApplyError;

use super::{MARKETPLACE, PLUGIN_NAME, io_err};

#[derive(Serialize)]
struct MarketplaceJson<'a> {
    name: &'a str,
    interface: MarketInterface<'a>,
    plugins: Vec<MarketPlugin<'a>>,
}

#[derive(Serialize)]
struct MarketInterface<'a> {
    #[serde(rename = "displayName")]
    display_name: &'a str,
}

#[derive(Serialize)]
struct MarketPlugin<'a> {
    name: &'a str,
    source: MarketSource<'a>,
    policy: MarketPolicy<'a>,
    category: &'a str,
}

#[derive(Serialize)]
struct MarketSource<'a> {
    source: &'a str,
    path: &'a str,
}

#[derive(Serialize)]
struct MarketPolicy<'a> {
    installation: &'a str,
    authentication: &'a str,
}


pub(super) fn write_marketplace_json(root: &Path) -> Result<(), ApplyError> {
    let dir = root.join(".agents").join("plugins");
    fs::create_dir_all(&dir).map_err(|e| io_err("create marketplace dir", &dir, e))?;
    let plugin_rel = format!("./plugins/{PLUGIN_NAME}");
    let value = MarketplaceJson {
        name: MARKETPLACE,
        interface: MarketInterface {
            display_name: "Systemprompt managed",
        },
        plugins: vec![MarketPlugin {
            name: PLUGIN_NAME,
            source: MarketSource {
                source: "local",
                path: &plugin_rel,
            },
            policy: MarketPolicy {
                installation: "INSTALLED_BY_DEFAULT",
                authentication: "ON_INSTALL",
            },
            category: "Productivity",
        }],
    };
    let bytes = serde_json::to_vec_pretty(&value).map_err(|e| ApplyError::Serialize {
        what: "codex marketplace.json".into(),
        source: e,
    })?;
    let path = dir.join("marketplace.json");
    fs::write(&path, bytes).map_err(|e| io_err("write marketplace.json", &path, e))
}

#[derive(Serialize)]
struct PluginJson<'a> {
    name: &'a str,
    version: &'a str,
    description: &'a str,
    skills: &'a str,
    interface: PluginInterface<'a>,
}

#[derive(Serialize)]
struct PluginInterface<'a> {
    #[serde(rename = "displayName")]
    display_name: &'a str,
}


pub(super) fn write_plugin_json(plugin_dir: &Path, version: &str) -> Result<(), ApplyError> {
    let dir = plugin_dir.join(".codex-plugin");
    fs::create_dir_all(&dir).map_err(|e| io_err("create .codex-plugin", &dir, e))?;
    let value = PluginJson {
        name: PLUGIN_NAME,
        version,
        description: "Skills managed by your systemprompt.io organization.",
        skills: "./skills/",
        interface: PluginInterface {
            display_name: "Systemprompt managed",
        },
    };
    let bytes = serde_json::to_vec_pretty(&value).map_err(|e| ApplyError::Serialize {
        what: "codex plugin.json".into(),
        source: e,
    })?;
    let path = dir.join("plugin.json");
    fs::write(&path, bytes).map_err(|e| io_err("write plugin.json", &path, e))
}

pub(super) fn read_existing_version(plugin_dir: &Path) -> Option<String> {
    let bytes = fs::read(plugin_dir.join(".codex-plugin").join("plugin.json")).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("version")?.as_str().map(str::to_owned)
}
