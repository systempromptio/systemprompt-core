use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::time::Duration;
use systemprompt_logging::CliService;
use systemprompt_models::validators::{ValidationConfigProvider, WebConfigRaw, WebMetadataRaw};
use systemprompt_models::{Config, ContentConfigRaw};
use systemprompt_traits::ConfigProvider;

pub fn load_content_config(
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
                    CliService::phase_warning("Content config", Some(&e));
                }
            },
        }
    }
    provider
}

pub fn load_web_config(
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
                    CliService::phase_warning("Web config", Some(&e));
                }
            },
        }
    }
    provider
}

pub fn load_web_metadata(
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
                    CliService::phase_warning("Web metadata", Some(&e));
                }
            },
        }
    }
    provider
}

pub fn create_spinner(message: &str) -> ProgressBar {
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

pub fn load_yaml_config<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
    serde_yaml::from_str(&content).map_err(|e| format!("Cannot parse {}: {}", path.display(), e))
}

pub fn load_extension_config(path: &Path) -> Result<serde_json::Value, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|e| format!("Cannot parse {}: {}", path.display(), e))?;
    serde_json::to_value(yaml).map_err(|e| format!("Cannot convert to JSON: {}", e))
}
