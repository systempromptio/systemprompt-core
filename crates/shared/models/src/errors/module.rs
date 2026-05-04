//! Errors raised while parsing or resolving module manifests.

#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error("Missing module dependencies: {0}")]
    MissingDependencies(String),

    #[error("Circular dependency detected in modules: {0}")]
    Cycle(String),
}
