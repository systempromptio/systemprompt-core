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
        let primary = self.bin.join(&exe_name);
        searched.push(primary.clone());

        let sibling = self.sibling_bin_path();
        let alt = sibling.as_ref().map(|s| s.join(&exe_name));
        if let Some(ref alt_path) = alt {
            searched.push(alt_path.clone());
        }

        match (primary.exists(), alt.as_ref().map_or(false, |p| p.exists())) {
            (true, true) => {
                let alt_path = alt.unwrap();
                let primary_mtime =
                    std::fs::metadata(&primary).and_then(|m| m.modified()).ok();
                let alt_mtime =
                    std::fs::metadata(&alt_path).and_then(|m| m.modified()).ok();
                match (primary_mtime, alt_mtime) {
                    (Some(p), Some(a)) if a > p => Self::ensure_absolute(alt_path),
                    _ => Self::ensure_absolute(primary),
                }
            }
            (true, false) => Self::ensure_absolute(primary),
            (false, true) => Self::ensure_absolute(alt.unwrap()),
            (false, false) => {
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
        }
    }

    fn sibling_bin_path(&self) -> Option<PathBuf> {
        let dir_name = self.bin.file_name()?.to_str()?;
        let sibling_name = match dir_name {
            "release" => "debug",
            "debug" => "release",
            _ => return None,
        };
        Some(self.bin.with_file_name(sibling_name))
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
