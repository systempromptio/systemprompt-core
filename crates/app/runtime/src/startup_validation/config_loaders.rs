//! Config-file loaders backing startup validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::time::Duration;
use systemprompt_logging::CliService;
use systemprompt_manifest::Config;
use systemprompt_manifest::validators::{ValidationConfigProvider, WebConfigRaw, WebMetadataRaw};
use systemprompt_models::ContentConfigRaw;
use systemprompt_traits::ConfigProvider;

#[derive(Debug, thiserror::Error)]
pub(super) enum ConfigFileError {
    #[error("Cannot read {}: {source}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Cannot parse {}: {source}", .path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("Cannot convert to JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(super) fn load_content_config(
    config: &Config,
    mut provider: ValidationConfigProvider,
    verbose: bool,
) -> ValidationConfigProvider {
    if let Some(content_config_path) = ConfigProvider::get(config, "content_config_path") {
        let spinner = if verbose {
            Some(create_spinner("Loading content config"))
        } else {
            None
        };
        match load_yaml_config::<ContentConfigRaw>(Path::new(&content_config_path)) {
            Ok(cfg) => {
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                if verbose {
                    CliService::phase_success("Content config", None);
                }
                provider = provider.with_content_config(cfg);
            },
            Err(e) => {
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                if verbose {
                    CliService::phase_warning("Content config", Some(&e.to_string()));
                }
            },
        }
    }
    provider
}

pub(super) fn load_web_config(
    config: &Config,
    mut provider: ValidationConfigProvider,
    verbose: bool,
) -> ValidationConfigProvider {
    if let Some(web_config_path) = ConfigProvider::get(config, "web_config_path") {
        let spinner = if verbose {
            Some(create_spinner("Loading web config"))
        } else {
            None
        };
        match load_yaml_config::<WebConfigRaw>(Path::new(&web_config_path)) {
            Ok(cfg) => {
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                if verbose {
                    CliService::phase_success("Web config", None);
                }
                provider = provider.with_web_config(cfg);
            },
            Err(e) => {
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                if verbose {
                    CliService::phase_warning("Web config", Some(&e.to_string()));
                }
            },
        }
    }
    provider
}

pub(super) fn load_web_metadata(
    config: &Config,
    mut provider: ValidationConfigProvider,
    verbose: bool,
) -> ValidationConfigProvider {
    if let Some(web_metadata_path) = ConfigProvider::get(config, "web_metadata_path") {
        let spinner = if verbose {
            Some(create_spinner("Loading web metadata"))
        } else {
            None
        };
        match load_yaml_config::<WebMetadataRaw>(Path::new(&web_metadata_path)) {
            Ok(cfg) => {
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                if verbose {
                    CliService::phase_success("Web metadata", None);
                }
                provider = provider.with_web_metadata(cfg);
            },
            Err(e) => {
                if let Some(s) = spinner {
                    s.finish_and_clear();
                }
                if verbose {
                    CliService::phase_warning("Web metadata", Some(&e.to_string()));
                }
            },
        }
    }
    provider
}

pub(super) fn create_spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner:.208} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    spinner.set_message(format!("{}...", message));
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

fn read_config_file(path: &Path) -> Result<String, ConfigFileError> {
    std::fs::read_to_string(path).map_err(|source| ConfigFileError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn parse_yaml<T: serde::de::DeserializeOwned>(
    path: &Path,
    content: &str,
) -> Result<T, ConfigFileError> {
    serde_yaml::from_str(content).map_err(|source| ConfigFileError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn load_yaml_config<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<T, ConfigFileError> {
    parse_yaml(path, &read_config_file(path)?)
}

// JSON: Extension config block from the profile YAML; the extension owns it.
pub(super) fn load_extension_config(path: &Path) -> Result<serde_json::Value, ConfigFileError> {
    let yaml: serde_yaml::Value = parse_yaml(path, &read_config_file(path)?)?;
    Ok(serde_json::to_value(yaml)?)
}
