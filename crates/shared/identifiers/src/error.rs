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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum IdValidationError {
    #[error("{id_type} cannot be empty")]
    Empty { id_type: &'static str },
    #[error("{id_type}: {message}")]
    Invalid {
        id_type: &'static str,
        message: String,
    },
    #[error("{id_type}: {source}")]
    Uuid {
        id_type: &'static str,
        #[source]
        source: uuid::Error,
    },
    #[error("{id_type}: {source}")]
    Json {
        id_type: &'static str,
        #[source]
        source: Arc<serde_json::Error>,
    },
}

impl PartialEq for IdValidationError {
    fn eq(&self, other: &Self) -> bool {
        self.comparable() == other.comparable()
    }
}

impl Eq for IdValidationError {}

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

    #[must_use]
    pub const fn uuid(id_type: &'static str, source: uuid::Error) -> Self {
        Self::Uuid { id_type, source }
    }

    #[must_use]
    pub fn json(id_type: &'static str, source: serde_json::Error) -> Self {
        Self::Json {
            id_type,
            source: Arc::new(source),
        }
    }

    fn comparable(&self) -> (u8, &'static str, String) {
        match self {
            Self::Empty { id_type } => (0, *id_type, String::new()),
            Self::Invalid { id_type, message } => (1, *id_type, message.clone()),
            Self::Uuid { id_type, source } => (2, *id_type, source.to_string()),
            Self::Json { id_type, source } => (3, *id_type, source.to_string()),
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
