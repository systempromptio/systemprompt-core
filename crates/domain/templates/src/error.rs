//! Error types for template operations.
//!
//! This module defines [`TemplateError`], the primary error type for template-related
//! failures including loading, compiling, and rendering templates.

use thiserror::Error;

/// Errors that can occur during template operations.
#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("Template not found: {0}")]
    NotFound(String),

    #[error("Failed to load template '{name}': {source}")]
    LoadError {
        name: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to compile template '{name}': {source}")]
    CompileError {
        name: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to render template '{name}': {source}")]
    RenderError {
        name: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("No loader available for template: {0}")]
    NoLoader(String),

    #[error("Template registry not initialized")]
    NotInitialized,
}
