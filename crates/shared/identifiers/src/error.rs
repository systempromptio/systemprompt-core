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

/// Failure raised by validating typed-identifier constructors.
///
/// The `id_type` field carries the originating Rust type name so that
/// composed error reports can attribute the failure to the correct
/// identifier without per-call-site formatting.
#[derive(Debug, Clone)]
pub enum IdValidationError {
    /// The supplied value was empty when the identifier requires a non-empty
    /// string.
    Empty {
        /// Rust type name of the identifier being constructed.
        id_type: &'static str,
    },
    /// The supplied value violated a custom validation rule.
    Invalid {
        /// Rust type name of the identifier being constructed.
        id_type: &'static str,
        /// Human-readable description of why the value was rejected.
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
    /// Constructs an `Empty` error for the given identifier type name.
    #[must_use]
    pub const fn empty(id_type: &'static str) -> Self {
        Self::Empty { id_type }
    }

    /// Constructs an `Invalid` error with a custom message.
    #[must_use]
    pub fn invalid(id_type: &'static str, message: impl Into<String>) -> Self {
        Self::Invalid {
            id_type,
            message: message.into(),
        }
    }
}

/// Failure raised by [`FromDbValue`](crate::FromDbValue) implementations.
#[derive(Debug, Clone, Error)]
pub enum DbValueError {
    /// A non-nullable target type was asked to materialise from a NULL
    /// database value.
    #[error("cannot convert NULL to {target}")]
    Null {
        /// Rust target type that was being constructed.
        target: &'static str,
    },
    /// The source variant cannot ever be coerced into the requested target.
    #[error("cannot convert {from} to {target}")]
    Incompatible {
        /// Source variant or category (e.g. `"Bytes"`).
        from: &'static str,
        /// Rust target type that was being constructed.
        target: &'static str,
    },
    /// The source string failed to parse into the requested target.
    #[error("cannot parse {value:?} as {target}")]
    Parse {
        /// Original string value that failed to parse.
        value: String,
        /// Rust target type that was being constructed.
        target: &'static str,
    },
    /// The numeric source value exceeds the range of the requested target.
    #[error("value out of range for {target}")]
    OutOfRange {
        /// Rust target type whose numeric range was exceeded.
        target: &'static str,
    },
}

impl DbValueError {
    /// Constructs a `Null` error for the given target type name.
    #[must_use]
    pub const fn null_for(target: &'static str) -> Self {
        Self::Null { target }
    }

    /// Constructs an `Incompatible` error.
    #[must_use]
    pub const fn incompatible(from: &'static str, target: &'static str) -> Self {
        Self::Incompatible { from, target }
    }

    /// Constructs a `Parse` error.
    #[must_use]
    pub fn parse(value: impl Into<String>, target: &'static str) -> Self {
        Self::Parse {
            value: value.into(),
            target,
        }
    }

    /// Constructs an `OutOfRange` error.
    #[must_use]
    pub const fn out_of_range(target: &'static str) -> Self {
        Self::OutOfRange { target }
    }
}
