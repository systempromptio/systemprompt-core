use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::validators::WebConfigRaw;
use systemprompt_models::Profile;

const DEFAULT_TEMPLATES_PATH: &str = "web/templates";
const DEFAULT_ASSETS_PATH: &str = "web/assets";

#[derive(Debug)]
pub struct WebPaths {
    pub templates: PathBuf,
    pub assets: PathBuf,
}

impl WebPaths {
    pub fn resolve() -> Result<Self> {
        let profile = ProfileBootstrap::get().context("Failed to get profile")?;
        Self::resolve_from_profile(profile)
    }

    pub fn resolve_from_profile(profile: &Profile) -> Result<Self> {
        let web_config_path = profile.paths.web_config();
        let services_path = &profile.paths.services;

        let (templates_path, assets_path) = match fs::read_to_string(&web_config_path) {
            Ok(content) => {
                let web_config: WebConfigRaw =
                    serde_yaml::from_str(&content).with_context(|| {
                        format!("Failed to parse web config at {}", web_config_path)
                    })?;

                match web_config.paths {
                    Some(paths) => (
                        paths
                            .templates
                            .unwrap_or_else(|| DEFAULT_TEMPLATES_PATH.to_string()),
                        paths
                            .assets
                            .unwrap_or_else(|| DEFAULT_ASSETS_PATH.to_string()),
                    ),
                    None => (
                        DEFAULT_TEMPLATES_PATH.to_string(),
                        DEFAULT_ASSETS_PATH.to_string(),
                    ),
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
                DEFAULT_TEMPLATES_PATH.to_string(),
                DEFAULT_ASSETS_PATH.to_string(),
            ),
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Failed to read web config at {}", web_config_path));
            },
        };

        let base = Path::new(services_path);

        let templates = if Path::new(&templates_path).is_absolute() {
            PathBuf::from(&templates_path)
        } else {
            base.join(&templates_path)
        };

        let assets = if Path::new(&assets_path).is_absolute() {
            PathBuf::from(&assets_path)
        } else {
            base.join(&assets_path)
        };

        Ok(Self { templates, assets })
    }
}
