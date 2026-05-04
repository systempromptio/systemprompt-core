//! Typed error surface for the templates crate.

use thiserror::Error;

/// Errors produced by template loading, compilation, rendering, and registry
/// lookup.
#[derive(Debug, Error)]
pub enum TemplateError {
    /// No template registered under the requested name.
    #[error("template not found: {0}")]
    NotFound(String),

    /// Loading a template from disk or an embedded source failed.
    #[error("failed to load template '{name}': {message}")]
    LoadError {
        /// Logical template name being loaded.
        name: String,
        /// Human-readable failure description.
        message: String,
    },

    /// Compiling a template into the underlying engine failed.
    #[error("failed to compile template '{name}': {message}")]
    CompileError {
        /// Logical template name being compiled.
        name: String,
        /// Human-readable failure description.
        message: String,
    },

    /// Rendering a previously-compiled template against a context failed.
    #[error("failed to render template '{name}': {message}")]
    RenderError {
        /// Logical template name being rendered.
        name: String,
        /// Human-readable failure description.
        message: String,
    },

    /// No loader is registered for the requested template.
    #[error("no loader available for template: {0}")]
    NoLoader(String),

    /// The template registry has not been initialised yet.
    #[error("template registry not initialized")]
    NotInitialized,

    /// I/O failure while discovering or reading template files.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Failure parsing the YAML template manifest.
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

/// Convenience alias for `Result<T, TemplateError>`.
pub type TemplateResult<T> = Result<T, TemplateError>;
