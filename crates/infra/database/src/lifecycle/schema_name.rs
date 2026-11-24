//! The Postgres schema an install targets.
//!
//! The installer, its freshness and residue audits and the existence probes
//! read the catalog of one named schema. [`SchemaName`] is that name,
//! validated as a plain lowercase Postgres identifier so it binds as a query
//! parameter and never needs quoting. The composition root that builds a real
//! install chooses it, with [`SchemaName::public`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

const MAX_IDENTIFIER_BYTES: usize = 63;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SchemaNameError {
    #[error("schema name is empty")]
    Empty,
    #[error("schema name {name:?} is longer than {MAX_IDENTIFIER_BYTES} bytes")]
    TooLong { name: String },
    #[error("schema name {name:?} is not a lowercase identifier ([a-z_][a-z0-9_]*)")]
    NotAnIdentifier { name: String },
}

/// A validated Postgres schema name an install targets.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SchemaName(String);

impl SchemaName {
    pub fn try_new(name: impl Into<String>) -> Result<Self, SchemaNameError> {
        let name = name.into();
        let mut bytes = name.bytes();
        let Some(first) = bytes.next() else {
            return Err(SchemaNameError::Empty);
        };
        if name.len() > MAX_IDENTIFIER_BYTES {
            return Err(SchemaNameError::TooLong { name });
        }
        let leading = first.is_ascii_lowercase() || first == b'_';
        if !leading || !bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
            return Err(SchemaNameError::NotAnIdentifier { name });
        }
        Ok(Self(name))
    }

    #[must_use]
    pub fn public() -> Self {
        Self("public".to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn qualify(&self, table: &str) -> String {
        if table.contains('.') {
            table.to_owned()
        } else {
            format!("{}.{table}", self.0)
        }
    }
}

impl fmt::Display for SchemaName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
