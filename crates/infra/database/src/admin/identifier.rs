use std::fmt;

use thiserror::Error;

const MAX_IDENTIFIER_LEN: usize = 63;

#[derive(Debug, Clone, Copy, Error)]
pub enum IdentifierError {
    #[error("identifier is empty")]
    Empty,
    #[error("identifier length {0} exceeds PostgreSQL limit of {MAX_IDENTIFIER_LEN}")]
    TooLong(usize),
    #[error("identifier must start with an ASCII letter or underscore")]
    BadLead,
    #[error("identifier contains invalid character {0:?}")]
    InvalidChar(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeIdentifier(String);

impl SafeIdentifier {
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

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SafeIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
