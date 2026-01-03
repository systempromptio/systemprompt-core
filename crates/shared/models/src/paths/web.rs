use std::path::{Path, PathBuf};

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

    pub fn from_profile(paths: &PathsConfig) -> Self {
        let root = PathBuf::from(paths.web_path_resolved());
        let dist = root.join(Self::DIST_DIR);

        Self {
            root,
            dist,
            config: PathBuf::from(paths.web_config()),
            metadata: PathBuf::from(paths.web_metadata()),
        }
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
