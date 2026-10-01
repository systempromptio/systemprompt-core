//! Typed error surface for the templates crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_template_provider::TemplateLoaderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template not found: {0}")]
    NotFound(String),

    #[error("failed to load template '{name}': {source}")]
    LoadError {
        name: String,
        #[source]
        source: TemplateLoaderError,
    },

    #[error("failed to load partial template '{}': {source}", path.display())]
    PartialRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to compile template '{name}': {message}")]
    CompileError { name: String, message: String },

    #[error("failed to render template '{name}': {source}")]
    RenderError {
        name: String,
        #[source]
        source: handlebars::RenderError,
    },

    #[error("no loader available for template: {0}")]
    NoLoader(String),

    #[error("template registry not initialized")]
    NotInitialized,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

pub type TemplateResult<T> = Result<T, TemplateError>;
