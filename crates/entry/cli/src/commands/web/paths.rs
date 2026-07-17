//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_config::ProfileBootstrap;
use systemprompt_models::Profile;
use systemprompt_models::validators::WebConfigRaw;

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
                            .unwrap_or_else(|| DEFAULT_TEMPLATES_PATH.to_owned()),
                        paths
                            .assets
                            .unwrap_or_else(|| DEFAULT_ASSETS_PATH.to_owned()),
                    ),
                    None => (
                        DEFAULT_TEMPLATES_PATH.to_owned(),
                        DEFAULT_ASSETS_PATH.to_owned(),
                    ),
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
                DEFAULT_TEMPLATES_PATH.to_owned(),
                DEFAULT_ASSETS_PATH.to_owned(),
            ),
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Failed to read web config at {}", web_config_path));
            },
        };

        let base = Path::new(services_path);
        let templates = normalize_under_services(&templates_path, base);
        let assets = normalize_under_services(&assets_path, base);

        Ok(Self { templates, assets })
    }
}

fn normalize_under_services(raw: &str, base: &Path) -> PathBuf {
    let candidate = Path::new(raw);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    let stripped = candidate.strip_prefix("services").unwrap_or(candidate);
    base.join(stripped)
}
