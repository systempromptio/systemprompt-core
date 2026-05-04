//! Semantic validation errors raised by the services / agents / plugins
//! / hooks / modules validation passes.

/// A semantic validation failure detected by the config / services
/// validation passes (agents, plugins, hooks, ports, secrets, modules).
///
/// The variants intentionally carry just enough context for log messages;
/// structural per-field errors are reported via the richer
/// [`systemprompt_traits::validation_report`] facility.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    /// A required configuration field was missing.
    #[error("{0}")]
    Required(String),

    /// A field value violated a constraint (length, charset, range…).
    #[error("{0}")]
    InvalidField(String),

    /// Two configuration entries collided on a port.
    #[error("{0}")]
    PortConflict(String),

    /// A reference pointed to an entity that was not declared.
    #[error("{0}")]
    UnknownReference(String),

    /// A circular dependency was detected between modules.
    #[error("{0}")]
    CircularDependency(String),

    /// A multi-cause aggregation rejected by an explicit business rule.
    #[error("{0}")]
    BusinessRule(String),
}

impl ConfigValidationError {
    /// Build a `Required` variant from a formatted message.
    #[must_use]
    pub fn required(msg: impl Into<String>) -> Self {
        Self::Required(msg.into())
    }

    /// Build an `InvalidField` variant from a formatted message.
    #[must_use]
    pub fn invalid_field(msg: impl Into<String>) -> Self {
        Self::InvalidField(msg.into())
    }

    /// Build a `PortConflict` variant from a formatted message.
    #[must_use]
    pub fn port_conflict(msg: impl Into<String>) -> Self {
        Self::PortConflict(msg.into())
    }

    /// Build an `UnknownReference` variant from a formatted message.
    #[must_use]
    pub fn unknown_reference(msg: impl Into<String>) -> Self {
        Self::UnknownReference(msg.into())
    }

    /// Build a `CircularDependency` variant from a formatted message.
    #[must_use]
    pub fn circular_dependency(msg: impl Into<String>) -> Self {
        Self::CircularDependency(msg.into())
    }

    /// Build a `BusinessRule` variant from a formatted message.
    #[must_use]
    pub fn business_rule(msg: impl Into<String>) -> Self {
        Self::BusinessRule(msg.into())
    }
}
