use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::config::Config;

const WEB_DIST_RELATIVE: &str = "core/web/dist";

static PATH_CONFIG: OnceLock<PathConfig> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct PathConfig {
    web_dist: PathBuf,
}

impl PathConfig {
    pub fn init() -> Result<()> {
        if PATH_CONFIG.get().is_some() {
            return Ok(());
        }
        let config = Self::from_config()?;
        let _ = PATH_CONFIG.set(config);
        Ok(())
    }

    pub fn get() -> Result<&'static Self> {
        PATH_CONFIG
            .get()
            .ok_or_else(|| anyhow!("PathConfig::init() not called"))
    }

    pub fn from_config() -> Result<Self> {
        let config = Config::get()?;
        let web_dist = PathBuf::from(&config.system_path).join(WEB_DIST_RELATIVE);
        Ok(Self { web_dist })
    }

    pub fn from_profile() -> Result<Self> {
        Self::from_config()
    }

    pub const fn web_dist(&self) -> &PathBuf {
        &self.web_dist
    }
}
