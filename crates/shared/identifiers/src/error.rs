//! Validation error types for identifiers.

use std::fmt;

#[derive(Debug, Clone)]
pub enum IdValidationError {
    Empty { id_type: &'static str },
    Invalid { id_type: &'static str, message: String },
}

impl fmt::Display for IdValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { id_type } => write!(f, "{} cannot be empty", id_type),
            Self::Invalid { id_type, message } => write!(f, "{}: {}", id_type, message),
        }
    }
}

impl std::error::Error for IdValidationError {}

impl IdValidationError {
    #[must_use]
    pub const fn empty(id_type: &'static str) -> Self {
        Self::Empty { id_type }
    }

    #[must_use]
    pub fn invalid(id_type: &'static str, message: impl Into<String>) -> Self {
        Self::Invalid {
            id_type,
            message: message.into(),
        }
    }
}
