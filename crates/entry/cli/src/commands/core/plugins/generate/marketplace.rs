//! Marketplace manifest generation for plugin bundles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use systemprompt_identifiers::PluginId;
use systemprompt_loader::ConfigLoader;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_manifest::{MarketplaceConfig, PluginConfig};
use systemprompt_models::bridge::plugin_bundle::{ManifestAuthor, PluginManifest};

pub(super) fn generate_marketplace_json(_plugins_path: &Path, system_path: &Path) -> Result<()> {
    let services = match ConfigLoader::load() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load services config; skipping marketplace generation");
            return Ok(());
        },
    };

    if services.marketplaces.is_empty() {
        tracing::info!(
            "No marketplaces declared in services config; skipping marketplace.json generation"
        );
        return Ok(());
    }

    let marketplace_dir = system_path.join(".claude-plugin");
    std::fs::create_dir_all(&marketplace_dir)?;

    let default_id = services
        .settings
        .default_marketplace_id
        .as_ref()
        .map(systemprompt_identifiers::MarketplaceId::as_str);

    for (id, marketplace) in &services.marketplaces {
        if !marketplace.enabled {
            continue;
        }

        let json = render_marketplace(id.as_str(), marketplace, &services);
        let content = serde_json::to_string_pretty(&json)?;

        let file_name = format!("marketplace-{}.json", id.as_str());
        std::fs::write(marketplace_dir.join(&file_name), &content)?;

        let is_default = default_id.map_or_else(|| id.as_str() == "default", |d| d == id.as_str());
        if is_default {
            std::fs::write(marketplace_dir.join("marketplace.json"), &content)?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedMarketplace {
    pub name: String,
    pub owner: GeneratedMarketplaceOwner,
    pub metadata: GeneratedMarketplaceMetadata,
    pub plugins: Vec<GeneratedMarketplacePlugin>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedMarketplaceOwner {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedMarketplaceMetadata {
    pub description: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedMarketplacePlugin {
    pub name: String,
    pub source: String,
    pub description: String,
    pub version: String,
}

pub fn render_marketplace(
    id: &str,
    marketplace: &MarketplaceConfig,
    services: &ServicesConfig,
) -> GeneratedMarketplace {
    let plugins = marketplace
        .plugins
        .include
        .iter()
        .map(|plugin_id| {
            let plugin = services.plugins.get(plugin_id);
            GeneratedMarketplacePlugin {
                name: plugin_id.clone(),
                source: format!("./storage/files/plugins/{plugin_id}"),
                description: plugin.map_or_else(String::new, |p| p.description.clone()),
                version: plugin.map_or_else(String::new, |p| p.version.clone()),
            }
        })
        .collect();

    GeneratedMarketplace {
        name: id.to_owned(),
        owner: GeneratedMarketplaceOwner {
            name: marketplace.author.name.clone(),
        },
        metadata: GeneratedMarketplaceMetadata {
            description: marketplace.description.clone(),
            version: marketplace.version.clone(),
        },
        plugins,
    }
}

pub fn generate_plugin_json(
    plugin: &PluginConfig,
    output_dir: &Path,
    files_generated: &mut Vec<String>,
) -> Result<()> {
    let claude_plugin_dir = output_dir.join(".claude-plugin");
    std::fs::create_dir_all(&claude_plugin_dir)?;

    let manifest = PluginManifest {
        name: plugin.id.as_str().to_owned(),
        description: plugin.description.clone(),
        version: plugin.version.clone(),
        author: Some(ManifestAuthor {
            name: plugin.author.name.clone(),
            email: plugin.author.email.clone(),
        }),
        hooks: None,
        keywords: plugin.keywords.clone(),
        installation_preference: None,
        ..PluginManifest::default()
    };

    let plugin_json_path = claude_plugin_dir.join("plugin.json");
    let content = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&plugin_json_path, content)?;
    files_generated.push(plugin_json_path.to_string_lossy().to_string());

    Ok(())
}

pub fn copy_scripts(
    plugin: &PluginConfig,
    plugins_path: &Path,
    plugin_id: &PluginId,
    output_dir: &Path,
    files_generated: &mut Vec<String>,
) -> Result<()> {
    if plugin.scripts.is_empty() {
        return Ok(());
    }

    let scripts_dir = output_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    for script in &plugin.scripts {
        let source_path = plugins_path.join(plugin_id.as_str()).join(&script.source);
        let dest_path = scripts_dir.join(&script.name);

        if source_path.exists() {
            std::fs::copy(&source_path, &dest_path)?;
            files_generated.push(dest_path.to_string_lossy().to_string());
        }
    }

    Ok(())
}
