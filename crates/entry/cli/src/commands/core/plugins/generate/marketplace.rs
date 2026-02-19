use anyhow::Result;
use std::path::Path;
use systemprompt_models::{PluginConfig, PluginConfigFile};

pub fn generate_marketplace_json(plugins_path: &Path, system_path: &Path) -> Result<()> {
    if !plugins_path.exists() {
        return Ok(());
    }

    let mut plugin_entries = Vec::new();

    for entry in std::fs::read_dir(plugins_path)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let config_path = path.join("config.yaml");
        if !config_path.exists() {
            continue;
        }

        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %config_path.display(), error = %e, "Failed to read plugin config");
                continue;
            },
        };

        let plugin_file: PluginConfigFile = match serde_yaml::from_str(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %config_path.display(), error = %e, "Failed to parse plugin config");
                continue;
            },
        };

        let plugin = &plugin_file.plugin;
        if !plugin.enabled {
            continue;
        }

        let dir_name = entry.file_name().to_string_lossy().to_string();

        plugin_entries.push(serde_json::json!({
            "name": plugin.id,
            "source": format!("./storage/files/plugins/{}", dir_name),
            "description": plugin.description,
            "version": plugin.version
        }));
    }

    let marketplace = serde_json::json!({
        "name": "systemprompt-marketplace",
        "owner": { "name": "systemprompt.io" },
        "metadata": {
            "description": "systemprompt.io plugin marketplace",
            "version": "0.1.0"
        },
        "plugins": plugin_entries
    });

    let marketplace_dir = system_path.join(".claude-plugin");
    std::fs::create_dir_all(&marketplace_dir)?;

    let marketplace_path = marketplace_dir.join("marketplace.json");
    let content = serde_json::to_string_pretty(&marketplace)?;
    std::fs::write(&marketplace_path, content)?;

    Ok(())
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
        serde_json::Value::String(plugin.id.clone()),
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
