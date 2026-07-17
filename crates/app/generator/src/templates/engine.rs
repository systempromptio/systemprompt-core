//! Tera engine construction with web-config path validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_models::{AppPaths, Config, WebConfig, WebConfigError};
use tokio::fs;

pub async fn load_web_config(paths: &AppPaths) -> Result<WebConfig, WebConfigError> {
    let config = Config::get().map_err(|e| WebConfigError::InvalidValue {
        field: "config".to_owned(),
        message: e.to_string(),
    })?;

    let content = fs::read_to_string(&config.web_config_path)
        .await
        .map_err(|e| WebConfigError::Io {
            path: config.web_config_path.clone(),
            source: e,
        })?;

    let mut web_config: WebConfig =
        serde_yaml::from_str(&content).map_err(WebConfigError::Parse)?;

    web_config.paths.resolve_relative_to(paths.system().root());
    validate_paths(&web_config)?;

    Ok(web_config)
}

fn validate_paths(config: &WebConfig) -> Result<(), WebConfigError> {
    let templates_path = Path::new(&config.paths.templates);

    if !config.paths.templates.is_empty() && !templates_path.exists() {
        return Err(WebConfigError::PathNotFound {
            field: "paths.templates".to_owned(),
            path: templates_path.to_path_buf(),
        });
    }

    Ok(())
}

pub fn get_templates_path(config: &WebConfig, paths: &AppPaths) -> PathBuf {
    let configured = &config.paths.templates;
    if !configured.is_empty() {
        let path = Path::new(configured);
        if path.exists() {
            return path.to_path_buf();
        }
    }

    paths.web().root().join("templates")
}
