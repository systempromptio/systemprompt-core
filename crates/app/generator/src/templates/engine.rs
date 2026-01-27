use std::path::{Path, PathBuf};

use systemprompt_models::{AppPaths, Config, FullWebConfig, WebConfigError};
use tokio::fs;

pub async fn load_web_config() -> Result<FullWebConfig, WebConfigError> {
    let config = Config::get().map_err(|e| WebConfigError::InvalidValue {
        field: "config".to_string(),
        message: e.to_string(),
    })?;
    let web_config_path = &config.web_config_path;

    let content = fs::read_to_string(web_config_path)
        .await
        .map_err(|e| WebConfigError::Io {
            path: web_config_path.clone(),
            source: e,
        })?;

    serde_yaml::from_str(&content).map_err(WebConfigError::Parse)
}

pub fn get_templates_path(config: &FullWebConfig) -> PathBuf {
    let configured = &config.paths.templates;
    if !configured.is_empty() && Path::new(configured).exists() {
        return PathBuf::from(configured);
    }

    AppPaths::get()
        .map(|p| p.web().root().join("templates"))
        .unwrap_or_else(|_| PathBuf::from("templates"))
}

pub fn get_assets_path(config: &FullWebConfig) -> &str {
    &config.paths.assets
}
