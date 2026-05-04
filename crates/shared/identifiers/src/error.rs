//! Error types raised by identifier validation and database value conversion.
//!
//! The crate exposes two error enums:
//!
//! - `IdValidationError` — produced when a `try_new` constructor on a typed
//!   identifier rejects its input (empty string, malformed shape, etc.).
//! - `DbValueError` — produced when `FromDbValue` cannot convert a `DbValue`
//!   variant into the requested target type (NULL where a value is required,
//!   type mismatch, parse failure, numeric overflow).
//!
//! Both implement `std::error::Error` so callers can compose them into
//! larger `thiserror`-derived enums via `#[from]`.

use std::fmt;

use thiserror::Error;

#[derive(Debug, Clone)]
pub enum IdValidationError {
    Empty {
        id_type: &'static str,
    },
    Invalid {
        id_type: &'static str,
        message: String,
    },
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

#[derive(Debug, Clone, Error)]
pub enum DbValueError {
    #[error("cannot convert NULL to {target}")]
    Null { target: &'static str },
    #[error("cannot convert {from} to {target}")]
    Incompatible {
        from: &'static str,
        target: &'static str,
    },
    #[error("cannot parse {value:?} as {target}")]
    Parse { value: String, target: &'static str },
    #[error("value out of range for {target}")]
    OutOfRange { target: &'static str },
}

impl DbValueError {
    #[must_use]
    pub const fn null_for(target: &'static str) -> Self {
        Self::Null { target }
    }

    #[must_use]
    pub const fn incompatible(from: &'static str, target: &'static str) -> Self {
        Self::Incompatible { from, target }
    }

    #[must_use]
    pub fn parse(value: impl Into<String>, target: &'static str) -> Self {
        Self::Parse {
            value: value.into(),
            target,
        }
    }

    #[must_use]
    pub const fn out_of_range(target: &'static str) -> Self {
        Self::OutOfRange { target }
    }
}
