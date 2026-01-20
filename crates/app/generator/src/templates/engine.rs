use anyhow::{anyhow, Result};
use tokio::fs;

pub async fn load_web_config() -> Result<serde_yaml::Value> {
    let web_config_path = std::env::var("SYSTEMPROMPT_WEB_CONFIG_PATH")
        .ok()
        .or_else(|| {
            systemprompt_models::Config::get()
                .ok()
                .map(|c| c.web_config_path.clone())
        })
        .ok_or_else(|| {
            anyhow!(
                "Web config path not available: set SYSTEMPROMPT_WEB_CONFIG_PATH or initialize \
                 Config"
            )
        })?;

    let content = fs::read_to_string(&web_config_path)
        .await
        .map_err(|e| anyhow!("Failed to read web config at '{}': {}", web_config_path, e))?;

    serde_yaml::from_str(&content).map_err(|e| anyhow!("Failed to parse web config: {}", e))
}

pub fn get_templates_path(config: &serde_yaml::Value) -> Result<String> {
    if let Ok(path) = std::env::var("SYSTEMPROMPT_TEMPLATES_PATH") {
        return Ok(path);
    }

    config
        .get("paths")
        .and_then(|p| p.get("templates"))
        .and_then(|t| t.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("paths.templates must be set in web config"))
}

pub fn get_assets_path(config: &serde_yaml::Value) -> Result<String> {
    if let Ok(path) = std::env::var("SYSTEMPROMPT_ASSETS_PATH") {
        return Ok(path);
    }

    config
        .get("paths")
        .and_then(|p| p.get("assets"))
        .and_then(|t| t.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("paths.assets must be set in web config"))
}
