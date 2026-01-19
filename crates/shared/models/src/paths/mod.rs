mod build;
pub mod constants;
mod error;
mod system;
mod web;

pub use build::BuildPaths;
pub use constants::{cloud_container, dir_names, file_names};
pub use error::PathError;
pub use system::SystemPaths;
pub use web::WebPaths;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::profile::PathsConfig;

static APP_PATHS: OnceLock<AppPaths> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AppPaths {
    system: SystemPaths,
    web: WebPaths,
    build: BuildPaths,
    storage: Option<PathBuf>,
    ai_config: Option<PathBuf>,
}

impl AppPaths {
    pub fn init(profile_paths: &PathsConfig) -> Result<(), PathError> {
        if APP_PATHS.get().is_some() {
            return Ok(());
        }
        let paths = Self::from_profile(profile_paths)?;
        APP_PATHS
            .set(paths)
            .map_err(|_| PathError::AlreadyInitialized)?;
        Ok(())
    }

    pub fn get() -> Result<&'static Self, PathError> {
        APP_PATHS.get().ok_or(PathError::NotInitialized)
    }

    fn from_profile(paths: &PathsConfig) -> Result<Self, PathError> {
        Ok(Self {
            system: SystemPaths::from_profile(paths)?,
            web: WebPaths::from_profile(paths),
            build: BuildPaths::from_profile(paths),
            storage: paths.storage.as_ref().map(PathBuf::from),
            ai_config: Some(PathBuf::from(paths.ai_config())),
        })
    }

    pub const fn system(&self) -> &SystemPaths {
        &self.system
    }

    pub const fn web(&self) -> &WebPaths {
        &self.web
    }

    pub const fn build(&self) -> &BuildPaths {
        &self.build
    }

    pub fn storage(&self) -> Option<&Path> {
        self.storage.as_deref()
    }

    pub fn ai_config(&self) -> Option<&Path> {
        self.ai_config.as_deref()
    }
}
