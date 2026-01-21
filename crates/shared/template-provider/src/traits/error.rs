use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TemplateLoaderError {
    #[error("Template not found: {0}")]
    NotFound(PathBuf),

    #[error("Directory traversal not allowed: {0}")]
    DirectoryTraversal(PathBuf),

    #[error("Path not within allowed directories: {0}")]
    OutsideBasePath(PathBuf),

    #[error("Cannot load single template from directory: {0}")]
    DirectoryNotSupported(PathBuf),

    #[error("Directory loading not supported by this loader")]
    DirectoryLoadingUnsupported,

    #[error("Invalid template name encoding: {0}")]
    InvalidEncoding(PathBuf),

    #[error("No base paths configured")]
    NoBasePaths,

    #[error("IO error for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Loader only handles embedded templates")]
    EmbeddedOnly,
}

impl TemplateLoaderError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, TemplateLoaderError>;
