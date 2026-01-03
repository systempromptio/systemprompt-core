use std::path::{Path, PathBuf};

use super::PathError;
use crate::profile::PathsConfig;

#[derive(Debug, Clone)]
pub struct BuildPaths {
    bin: PathBuf,
}

impl BuildPaths {
    pub fn from_profile(paths: &PathsConfig) -> Self {
        Self {
            bin: PathBuf::from(&paths.bin),
        }
    }

    pub fn resolve_binary(&self, name: &str) -> Result<PathBuf, PathError> {
        let path = self.bin.join(name);
        if path.exists() {
            Self::ensure_absolute(path)
        } else {
            Err(PathError::BinaryNotFound {
                name: name.to_string(),
                searched: vec![path],
            })
        }
    }

    fn ensure_absolute(path: PathBuf) -> Result<PathBuf, PathError> {
        if path.is_absolute() {
            Ok(path)
        } else {
            std::fs::canonicalize(&path).map_err(|source| PathError::CanonicalizeFailed {
                path,
                field: "binary",
                source,
            })
        }
    }

    pub fn binary_exists(&self, name: &str) -> bool {
        self.resolve_binary(name).is_ok()
    }

    pub fn bin(&self) -> &Path {
        &self.bin
    }
}
