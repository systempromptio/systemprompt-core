//! Web template/asset path resolution under the services root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_loader::BootContext;
use systemprompt_manifest::Profile;
use systemprompt_manifest::validators::WebConfigRaw;

const DEFAULT_TEMPLATES_PATH: &str = "web/templates";
const DEFAULT_ASSETS_PATH: &str = "web/assets";

#[derive(Debug)]
pub struct WebPaths {
    pub templates: PathBuf,
    pub assets: PathBuf,
}

impl WebPaths {
    pub fn resolve(boot: &BootContext) -> Result<Self> {
        Self::resolve_from_services_root(&boot.require_services_root().path)
    }

    pub fn resolve_from_profile(profile: &Profile) -> Result<Self> {
        Self::resolve_from_services_root(Path::new(&profile.paths.services))
    }

    fn resolve_from_services_root(services_root: &Path) -> Result<Self> {
        let web_config_path = services_root.join("web/config.yaml");

        let (templates_path, assets_path) = match fs::read_to_string(&web_config_path) {
            Ok(content) => {
                let web_config: WebConfigRaw =
                    serde_yaml::from_str(&content).with_context(|| {
                        format!(
                            "Failed to parse web config at {}",
                            web_config_path.display()
                        )
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
                return Err(e).with_context(|| {
                    format!("Failed to read web config at {}", web_config_path.display())
                });
            },
        };

        let base = services_root;
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
