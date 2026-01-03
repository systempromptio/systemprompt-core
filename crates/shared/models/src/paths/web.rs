use std::path::{Path, PathBuf};

use super::PathError;
use crate::profile::PathsConfig;

#[derive(Debug, Clone)]
pub struct WebPaths {
    root: PathBuf,
    dist: PathBuf,
    config: PathBuf,
    metadata: PathBuf,
}

impl WebPaths {
    const DIST_DIR: &'static str = "dist";

    pub fn from_profile(paths: &PathsConfig) -> Result<Self, PathError> {
        let root = PathBuf::from(paths.web_path_resolved());
        let dist = root.join(Self::DIST_DIR);

        Ok(Self {
            root,
            dist,
            config: PathBuf::from(paths.web_config()),
            metadata: PathBuf::from(paths.web_metadata()),
        })
    }

    fn require_path(path: Option<&str>, field: &'static str) -> Result<PathBuf, PathError> {
        path.map(PathBuf::from)
            .ok_or(PathError::NotConfigured { field })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dist(&self) -> &Path {
        &self.dist
    }

    pub fn config(&self) -> &Path {
        &self.config
    }

    pub fn metadata(&self) -> &Path {
        &self.metadata
    }
}
