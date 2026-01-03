use std::path::{Path, PathBuf};

use super::PathError;
use crate::profile::PathsConfig;

#[derive(Debug, Clone)]
pub struct SystemPaths {
    root: PathBuf,
    services: PathBuf,
    skills: PathBuf,
    settings: PathBuf,
    content_config: PathBuf,
    geoip_database: Option<PathBuf>,
}

impl SystemPaths {
    const LOGS_DIR: &'static str = "logs";

    pub fn from_profile(paths: &PathsConfig) -> Result<Self, PathError> {
        let root = Self::canonicalize(&paths.system, "system")?;

        Ok(Self {
            root,
            services: PathBuf::from(&paths.services),
            skills: PathBuf::from(paths.skills()),
            settings: PathBuf::from(paths.config()),
            content_config: PathBuf::from(paths.content_config()),
            geoip_database: paths.geoip_database.as_ref().map(PathBuf::from),
        })
    }

    fn canonicalize(path: &str, field: &'static str) -> Result<PathBuf, PathError> {
        std::fs::canonicalize(path).map_err(|source| PathError::CanonicalizeFailed {
            path: PathBuf::from(path),
            field,
            source,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn services(&self) -> &Path {
        &self.services
    }

    pub fn skills(&self) -> &Path {
        &self.skills
    }

    pub fn settings(&self) -> &Path {
        &self.settings
    }

    pub fn content_config(&self) -> &Path {
        &self.content_config
    }

    pub fn geoip_database(&self) -> Option<&Path> {
        self.geoip_database.as_deref()
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join(Self::LOGS_DIR)
    }

    pub fn resolve_skill(&self, name: &str) -> PathBuf {
        self.skills.join(name)
    }

    pub fn resolve_service(&self, name: &str) -> PathBuf {
        self.services.join(name)
    }
}
