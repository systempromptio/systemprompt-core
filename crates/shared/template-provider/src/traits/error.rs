use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by [`TemplateLoader`](super::TemplateLoader)
/// implementations.
///
/// The variants distinguish between configuration failures (no base paths,
/// embedded-only loader receiving a file source), security checks
/// (directory traversal, paths outside the configured sandbox), and underlying
/// I/O. All variants carry enough context to identify the offending path.
#[derive(Error, Debug)]
pub enum TemplateLoaderError {
    /// The requested template file was not found in any configured base path.
    #[error("Template not found: {0}")]
    NotFound(PathBuf),

    /// The path contained `..` components and was rejected before any I/O.
    #[error("Directory traversal not allowed: {0}")]
    DirectoryTraversal(PathBuf),

    /// The path canonicalised to a location outside every configured base path.
    #[error("Path not within allowed directories: {0}")]
    OutsideBasePath(PathBuf),

    /// `load()` was called with a `TemplateSource::Directory`, which is not
    /// supported — use `load_directory()` instead.
    #[error("Cannot load single template from directory: {0}")]
    DirectoryNotSupported(PathBuf),

    /// The loader implementation does not support directory enumeration.
    #[error("Directory loading not supported by this loader")]
    DirectoryLoadingUnsupported,

    /// A directory entry's filename was not valid UTF-8.
    #[error("Invalid template name encoding: {0}")]
    InvalidEncoding(PathBuf),

    /// The loader was constructed with an empty base-path list and a relative
    /// or directory request was received.
    #[error("No base paths configured")]
    NoBasePaths,

    /// Underlying I/O failure while reading or canonicalising `path`.
    #[error("IO error for {path}: {source}")]
    Io {
        /// Path that triggered the I/O failure.
        path: PathBuf,
        /// The originating `std::io::Error`.
        #[source]
        source: std::io::Error,
    },

    /// An [`EmbeddedLoader`](super::EmbeddedLoader) received a non-embedded
    /// `TemplateSource`.
    #[error("Loader only handles embedded templates")]
    EmbeddedOnly,
}

impl TemplateLoaderError {
    /// Construct an [`TemplateLoaderError::Io`] from a path and
    /// `std::io::Error`.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

/// `Result` alias used by [`TemplateLoader`](super::TemplateLoader) methods.
pub type Result<T> = std::result::Result<T, TemplateLoaderError>;
