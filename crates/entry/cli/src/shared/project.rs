use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("Not a systemprompt.io project: {path}\n\nLooking for .systemprompt directory alongside Cargo.toml, services/, or storage/")]
    ProjectNotFound { path: PathBuf },

    #[error("Failed to resolve path {path}: {source}")]
    PathResolution {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn is_valid_project_root(path: &Path) -> bool {
    if !path.join(".systemprompt").is_dir() {
        return false;
    }
    path.join("Cargo.toml").exists()
        || path.join("services").is_dir()
        || path.join("storage").is_dir()
}

#[derive(Debug, Clone)]
pub struct ProjectRoot(PathBuf);

impl ProjectRoot {
    pub fn discover() -> Result<Self, ProjectError> {
        let current = std::env::current_dir().map_err(|e| ProjectError::PathResolution {
            path: PathBuf::from("."),
            source: e,
        })?;

        if is_valid_project_root(&current) {
            return Ok(Self(current));
        }

        let mut search = current.as_path();
        while let Some(parent) = search.parent() {
            if is_valid_project_root(parent) {
                return Ok(Self(parent.to_path_buf()));
            }
            search = parent;
        }

        Err(ProjectError::ProjectNotFound { path: current })
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for ProjectRoot {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}
