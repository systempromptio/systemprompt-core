//! Validated `PostgreSQL` identifier wrapper used wherever a raw table name or
//! column name flows from user input into a SQL string.

use std::fmt;

use thiserror::Error;

const MAX_IDENTIFIER_LEN: usize = 63;

/// Reasons [`SafeIdentifier::parse`] can reject input.
#[derive(Debug, Clone, Copy, Error)]
pub enum IdentifierError {
    /// Input was empty.
    #[error("identifier is empty")]
    Empty,
    /// Input exceeds `PostgreSQL`s 63-byte identifier limit.
    #[error("identifier length {0} exceeds `PostgreSQL` limit of {MAX_IDENTIFIER_LEN}")]
    TooLong(usize),
    /// First character is not an ASCII letter or underscore.
    #[error("identifier must start with an ASCII letter or underscore")]
    BadLead,
    /// Input contains a non-alphanumeric, non-underscore byte.
    #[error("identifier contains invalid character {0:?}")]
    InvalidChar(char),
}

/// Validated identifier guaranteed to satisfy `PostgreSQL`s unquoted-identifier
/// rules. Constructed via [`SafeIdentifier::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeIdentifier(String);

impl SafeIdentifier {
    /// Parse and validate `raw`. Returns [`IdentifierError`] if `raw` does
    /// not satisfy `[A-Za-z_][A-Za-z0-9_]{0,62}`.
    pub fn parse(raw: &str) -> Result<Self, IdentifierError> {
        if raw.is_empty() {
            return Err(IdentifierError::Empty);
        }
        if raw.len() > MAX_IDENTIFIER_LEN {
            return Err(IdentifierError::TooLong(raw.len()));
        }
        let mut chars = raw.chars();
        let first = chars.next().ok_or(IdentifierError::Empty)?;
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(IdentifierError::BadLead);
        }
        for c in chars {
            if !(c.is_ascii_alphanumeric() || c == '_') {
                return Err(IdentifierError::InvalidChar(c));
            }
        }
        Ok(Self(raw.to_string()))
    }

    /// Borrow the underlying validated identifier string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SafeIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
