mod build;
pub mod constants;
mod error;
mod storage;
mod system;
mod web;

pub use build::BuildPaths;
pub use constants::{cloud_container, dir_names, file_names};
pub use error::PathError;
pub use storage::StoragePaths;
pub use system::SystemPaths;
pub use web::WebPaths;

use std::path::Path;
use std::sync::OnceLock;

use crate::profile::PathsConfig;
use systemprompt_extension::AssetPaths;

static APP_PATHS: OnceLock<AppPaths> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AppPaths {
    system: SystemPaths,
    web: WebPaths,
    build: BuildPaths,
    storage: StoragePaths,
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
            storage: StoragePaths::from_profile(paths)?,
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

    pub const fn storage(&self) -> &StoragePaths {
        &self.storage
    }
}

impl AssetPaths for AppPaths {
    fn storage_files(&self) -> &Path {
        self.storage.files()
    }

    fn web_dist(&self) -> &Path {
        self.web.dist()
    }
}
