use anyhow::{anyhow, Result};
use systemprompt_models::Config;
use tokio::fs;

pub async fn load_web_config() -> Result<serde_yaml::Value> {
    let config = Config::get()?;
    let web_config_path = &config.web_config_path;

    let content = fs::read_to_string(web_config_path)
        .await
        .map_err(|e| anyhow!("Failed to read web config at '{}': {}", web_config_path, e))?;

    serde_yaml::from_str(&content).map_err(|e| anyhow!("Failed to parse web config: {}", e))
}

pub fn get_templates_path(config: &serde_yaml::Value) -> Result<String> {
    config
        .get("paths")
        .and_then(|p| p.get("templates"))
        .and_then(|t| t.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("paths.templates must be set in web config"))
}

pub fn get_assets_path(config: &serde_yaml::Value) -> Result<String> {
    config
        .get("paths")
        .and_then(|p| p.get("assets"))
        .and_then(|t| t.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("paths.assets must be set in web config"))
}
