use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::profile_bootstrap::ProfileBootstrap;

static PATH_CONFIG: OnceLock<PathConfig> = OnceLock::new();

fn profile_error(field: &str, message: &str) -> anyhow::Error {
    let profile_path = ProfileBootstrap::get_path()
        .map(ToString::to_string)
        .unwrap_or_else(|_| "<not set>".to_string());

    anyhow!(
        "Profile Error: {}\n\n  Field: paths.{}\n  Profile: {}\n\n  To fix:\n  - Run \
         'systemprompt cloud config' to regenerate profile\n  - Or manually add paths.{} to your \
         profile",
        message,
        field,
        profile_path,
        field
    )
}

#[derive(Debug, Clone)]
pub struct PathConfig {
    web_dist: PathBuf,
}

impl PathConfig {
    pub fn init() -> Result<()> {
        if PATH_CONFIG.get().is_some() {
            return Ok(());
        }
        let config = Self::from_profile()?;
        config.validate()?;
        let _ = PATH_CONFIG.set(config);
        Ok(())
    }

    pub fn get() -> Result<&'static Self> {
        PATH_CONFIG
            .get()
            .ok_or_else(|| anyhow!("PathConfig::init() not called"))
    }

    pub fn from_profile() -> Result<Self> {
        let profile =
            ProfileBootstrap::get().map_err(|e| anyhow!("Profile not initialized: {}", e))?;

        let web_dist = profile
            .paths
            .web_dist
            .as_ref()
            .ok_or_else(|| profile_error("web_dist", "Required path not configured"))?;

        Ok(Self {
            web_dist: PathBuf::from(web_dist),
        })
    }

    pub fn validate(&self) -> Result<()> {
        if !self.web_dist.is_absolute() {
            return Err(profile_error(
                "web_dist",
                &format!("Must be an absolute path, got: {}", self.web_dist.display()),
            ));
        }
        Ok(())
    }

    pub const fn web_dist(&self) -> &PathBuf {
        &self.web_dist
    }
}
