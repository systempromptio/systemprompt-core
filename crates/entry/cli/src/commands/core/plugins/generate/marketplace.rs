use anyhow::Result;
use std::path::Path;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::services::ServicesConfig;
use systemprompt_models::{MarketplaceConfig, PluginConfig};

pub fn generate_marketplace_json(_plugins_path: &Path, system_path: &Path) -> Result<()> {
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

    let default_id = services.settings.default_marketplace_id.as_deref();

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

fn render_marketplace(
    id: &str,
    marketplace: &MarketplaceConfig,
    services: &ServicesConfig,
) -> serde_json::Value {
    let plugin_entries: Vec<serde_json::Value> = marketplace
        .plugins
        .include
        .iter()
        .map(|plugin_id| {
            let plugin = services.plugins.get(plugin_id);
            serde_json::json!({
                "name": plugin_id,
                "source": format!("./storage/files/plugins/{plugin_id}"),
                "description": plugin.map(|p| p.description.clone()).unwrap_or_else(String::new),
                "version": plugin.map(|p| p.version.clone()).unwrap_or_else(String::new),
            })
        })
        .collect();

    serde_json::json!({
        "name": id,
        "owner": { "name": marketplace.author.name.clone() },
        "metadata": {
            "description": marketplace.description.clone(),
            "version": marketplace.version.clone(),
        },
        "plugins": plugin_entries,
    })
}

pub fn generate_plugin_json(
    plugin: &PluginConfig,
    output_dir: &Path,
    files_generated: &mut Vec<String>,
) -> Result<()> {
    let claude_plugin_dir = output_dir.join(".claude-plugin");
    std::fs::create_dir_all(&claude_plugin_dir)?;

    let mut manifest = serde_json::Map::new();
    manifest.insert(
        "name".to_string(),
        serde_json::Value::String(plugin.id.to_string()),
    );
    manifest.insert(
        "description".to_string(),
        serde_json::Value::String(plugin.description.clone()),
    );
    manifest.insert(
        "version".to_string(),
        serde_json::Value::String(plugin.version.clone()),
    );

    let mut author_obj = serde_json::Map::new();
    author_obj.insert(
        "name".to_string(),
        serde_json::Value::String(plugin.author.name.clone()),
    );
    manifest.insert("author".to_string(), serde_json::Value::Object(author_obj));

    if !plugin.keywords.is_empty() {
        let keywords: Vec<serde_json::Value> = plugin
            .keywords
            .iter()
            .map(|k| serde_json::Value::String(k.clone()))
            .collect();
        manifest.insert("keywords".to_string(), serde_json::Value::Array(keywords));
    }

    let plugin_json_path = claude_plugin_dir.join("plugin.json");
    let content = serde_json::to_string_pretty(&serde_json::Value::Object(manifest))?;
    std::fs::write(&plugin_json_path, content)?;
    files_generated.push(plugin_json_path.to_string_lossy().to_string());

    Ok(())
}

pub fn copy_scripts(
    plugin: &PluginConfig,
    plugins_path: &Path,
    plugin_id: &str,
    output_dir: &Path,
    files_generated: &mut Vec<String>,
) -> Result<()> {
    if plugin.scripts.is_empty() {
        return Ok(());
    }

    let scripts_dir = output_dir.join("scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    for script in &plugin.scripts {
        let source_path = plugins_path.join(plugin_id).join(&script.source);
        let dest_path = scripts_dir.join(&script.name);

        if source_path.exists() {
            std::fs::copy(&source_path, &dest_path)?;
            files_generated.push(dest_path.to_string_lossy().to_string());
        }
    }

    Ok(())
}
