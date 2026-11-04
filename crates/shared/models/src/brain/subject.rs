//! The business subject a fact is about, written `<kind>:<id>`, such as
//! `client:acme` or `service:billing`.
//!
//! Both halves are bounded ([`MAX_SUBJECT_KIND_LEN`],
//! [`MAX_SUBJECT_ID_LEN`]) to what the subject table admits, so a subject
//! that could never be indexed is refused where it enters rather than when
//! a consumer round tries to write its row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Why: Postgres — brain_subjects carries CHECK (length(subject_kind)
// BETWEEN 1 AND 64) and CHECK (length(subject_id) BETWEEN 1 AND 255),
// counted in characters; brain_facts carries no such check.
pub const MAX_SUBJECT_KIND_LEN: usize = 64;
pub const MAX_SUBJECT_ID_LEN: usize = 255;

/// A business subject: the subject kind a fact kind declares and the
/// subject's key within it.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct SubjectKey {
    pub kind: String,
    pub id: String,
}

/// Why a subject key is not one the store admits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubjectKeyError {
    #[error("invalid subject key {0:?}: expected `<kind>:<id>`")]
    Shape(String),
    #[error(
        "subject kind is {len} characters; between 1 and {} are allowed",
        MAX_SUBJECT_KIND_LEN
    )]
    KindLength { len: usize },
    #[error(
        "subject id is {len} characters; between 1 and {} are allowed",
        MAX_SUBJECT_ID_LEN
    )]
    IdLength { len: usize },
}

impl SubjectKey {
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }

    pub fn try_new(
        kind: impl Into<String>,
        id: impl Into<String>,
    ) -> Result<Self, SubjectKeyError> {
        let key = Self::new(kind, id);
        key.validate()?;
        Ok(key)
    }

    pub fn validate(&self) -> Result<(), SubjectKeyError> {
        let kind = self.kind.chars().count();
        if !(1..=MAX_SUBJECT_KIND_LEN).contains(&kind) {
            return Err(SubjectKeyError::KindLength { len: kind });
        }
        let id = self.id.chars().count();
        if !(1..=MAX_SUBJECT_ID_LEN).contains(&id) {
            return Err(SubjectKeyError::IdLength { len: id });
        }
        Ok(())
    }
}

impl fmt::Display for SubjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.id)
    }
}

impl FromStr for SubjectKey {
    type Err = SubjectKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((kind, id)) if !kind.is_empty() && !id.is_empty() => Self::try_new(kind, id),
            _ => Err(SubjectKeyError::Shape(s.to_owned())),
        }
    }
}
