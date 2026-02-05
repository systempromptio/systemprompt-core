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
        let mut searched = Vec::new();

        let exe_name = format!("{}{}", name, std::env::consts::EXE_SUFFIX);
        let exe_path = self.bin.join(&exe_name);
        searched.push(exe_path.clone());

        if exe_path.exists() {
            return Self::ensure_absolute(exe_path);
        }

        if !std::env::consts::EXE_SUFFIX.is_empty() {
            let path = self.bin.join(name);
            searched.push(path.clone());
            if path.exists() {
                return Self::ensure_absolute(path);
            }
        }

        Err(PathError::BinaryNotFound {
            name: name.to_string(),
            searched,
        })
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
