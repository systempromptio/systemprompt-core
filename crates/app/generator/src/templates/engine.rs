use std::path::{Path, PathBuf};

use systemprompt_models::{AppPaths, Config, FullWebConfig, WebConfigError};
use tokio::fs;

pub async fn load_web_config() -> Result<FullWebConfig, WebConfigError> {
    let config = Config::get().map_err(|e| WebConfigError::InvalidValue {
        field: "config".to_string(),
        message: e.to_string(),
    })?;

    let content = fs::read_to_string(&config.web_config_path)
        .await
        .map_err(|e| WebConfigError::Io {
            path: config.web_config_path.clone(),
            source: e,
        })?;

    let mut web_config: FullWebConfig =
        serde_yaml::from_str(&content).map_err(WebConfigError::Parse)?;

    let paths = AppPaths::get().map_err(|e| WebConfigError::InvalidValue {
        field: "paths".to_string(),
        message: e.to_string(),
    })?;

    web_config.paths.resolve_relative_to(paths.system().root());
    validate_paths(&web_config)?;

    Ok(web_config)
}

fn validate_paths(config: &FullWebConfig) -> Result<(), WebConfigError> {
    let templates_path = Path::new(&config.paths.templates);

    if !config.paths.templates.is_empty() && !templates_path.exists() {
        return Err(WebConfigError::PathNotFound {
            field: "paths.templates".to_string(),
            path: templates_path.to_path_buf(),
        });
    }

    Ok(())
}

pub fn get_templates_path(config: &FullWebConfig) -> PathBuf {
    let configured = &config.paths.templates;
    if !configured.is_empty() {
        let path = Path::new(configured);
        if path.exists() {
            return path.to_path_buf();
        }
    }

    AppPaths::get().map_or_else(|_| PathBuf::from("templates"), |p| p.web().root().join("templates"))
}

pub fn get_assets_path(config: &FullWebConfig) -> &str {
    &config.paths.assets
}
