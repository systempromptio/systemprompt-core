use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("Not a systemprompt.io project: {path}\n\nLooking for .systemprompt directory")]
    ProjectNotFound { path: PathBuf },

    #[error("Failed to resolve path {path}: {source}")]
    PathResolution {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct ProjectRoot(PathBuf);

impl ProjectRoot {
    pub fn discover() -> Result<Self, ProjectError> {
        let current = std::env::current_dir().map_err(|e| ProjectError::PathResolution {
            path: PathBuf::from("."),
            source: e,
        })?;

        if current.join(".systemprompt").is_dir() {
            return Ok(Self(current));
        }

        let mut search = current.as_path();
        while let Some(parent) = search.parent() {
            if parent.join(".systemprompt").is_dir() {
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
