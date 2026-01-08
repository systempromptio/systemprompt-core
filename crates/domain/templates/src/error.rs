use thiserror::Error;

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
