//! Semantic validation errors raised by the services / agents / plugins
//! / hooks / modules validation passes.

#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("{0}")]
    Required(String),

    #[error("{0}")]
    InvalidField(String),

    #[error("{0}")]
    PortConflict(String),

    #[error("{0}")]
    UnknownReference(String),

    #[error("{0}")]
    CircularDependency(String),

    #[error("{0}")]
    BusinessRule(String),
}

impl ConfigValidationError {
    #[must_use]
    pub fn required(msg: impl Into<String>) -> Self {
        Self::Required(msg.into())
    }

    #[must_use]
    pub fn invalid_field(msg: impl Into<String>) -> Self {
        Self::InvalidField(msg.into())
    }

    #[must_use]
    pub fn port_conflict(msg: impl Into<String>) -> Self {
        Self::PortConflict(msg.into())
    }

    #[must_use]
    pub fn unknown_reference(msg: impl Into<String>) -> Self {
        Self::UnknownReference(msg.into())
    }

    #[must_use]
    pub fn circular_dependency(msg: impl Into<String>) -> Self {
        Self::CircularDependency(msg.into())
    }

    #[must_use]
    pub fn business_rule(msg: impl Into<String>) -> Self {
        Self::BusinessRule(msg.into())
    }
}
