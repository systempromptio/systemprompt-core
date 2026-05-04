//! Errors raised while parsing or resolving module manifests.

/// Failure to parse, order, or resolve a module manifest tree.
#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    /// The YAML document could not be parsed into a `Module`.
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    /// One or more declared dependencies were not present in the input set.
    #[error("Missing module dependencies: {0}")]
    MissingDependencies(String),

    /// The dependency graph contains a cycle.
    #[error("Circular dependency detected in modules: {0}")]
    Cycle(String),
}
