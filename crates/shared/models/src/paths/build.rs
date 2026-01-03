use std::path::{Path, PathBuf};

use super::PathError;
use crate::profile::PathsConfig;

#[derive(Debug, Clone)]
pub struct BuildPaths {
    target: PathBuf,
    mcp_override: Option<PathBuf>,
}

impl BuildPaths {
    const TARGET_DIR: &'static str = "target";
    const MCP_PATH_ENV: &'static str = "SYSTEMPROMPT_MCP_PATH";

    pub fn from_profile(paths: &PathsConfig) -> Self {
        let system_root = PathBuf::from(&paths.system);

        Self {
            target: system_root.join(Self::TARGET_DIR),
            mcp_override: std::env::var(Self::MCP_PATH_ENV).ok().map(PathBuf::from),
        }
    }

    pub fn resolve_binary(&self, name: &str) -> Result<PathBuf, PathError> {
        self.resolve_binary_with_crate(name, None)
    }

    pub fn resolve_binary_with_crate(
        &self,
        name: &str,
        crate_path: Option<&Path>,
    ) -> Result<PathBuf, PathError> {
        let mut searched = Vec::new();

        if let Some(ref mcp_path) = self.mcp_override {
            let path = mcp_path.join(name);
            searched.push(path.clone());
            if path.exists() {
                return Self::ensure_absolute(path);
            }
        }

        if let Some(crate_path) = crate_path {
            let release = crate_path.join("target/release").join(name);
            let debug = crate_path.join("target/debug").join(name);
            searched.push(release.clone());
            searched.push(debug.clone());
            if release.exists() {
                return Self::ensure_absolute(release);
            }
            if debug.exists() {
                return Self::ensure_absolute(debug);
            }
        }

        let release = self.target.join("release").join(name);
        let debug = self.target.join("debug").join(name);
        searched.push(release.clone());
        searched.push(debug.clone());

        match (release.exists(), debug.exists()) {
            (true, _) => Self::ensure_absolute(release),
            (_, true) => Self::ensure_absolute(debug),
            _ => Err(PathError::BinaryNotFound {
                name: name.to_string(),
                searched,
            }),
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

    pub fn target(&self) -> &Path {
        &self.target
    }

    pub fn release(&self) -> PathBuf {
        self.target.join("release")
    }

    pub fn debug(&self) -> PathBuf {
        self.target.join("debug")
    }
}
