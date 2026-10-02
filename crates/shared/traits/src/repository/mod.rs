//! The workspace's single repository error type.
//!
//! Every repository — infra or domain — returns [`RepositoryError`]. Its
//! variants carry the classification an HTTP or job boundary needs (missing
//! entity, conflict, constraint violation, corrupt stored data, backend
//! failure) and keep the underlying error as a `#[source]` rather than its
//! text. The caller-facing variants are structured: a missing entity or a
//! conflict names the entity kind and its key, an invalid argument or a
//! corrupt stored value names the field, and `reason` carries only what that
//! structure cannot. With the `sqlx` feature, `From<sqlx::Error>` classifies a
//! database error by SQLSTATE, so `?` on a query result yields `NotFound`,
//! `Constraint` or `Database` rather than an opaque string.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "sqlx")]
mod sqlx_classify;

use std::fmt;

pub type BoxedSource = Box<dyn std::error::Error + Send + Sync + 'static>;

/// The integrity constraint class a database rejected a write with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    Unique,
    Exclusion,
    ForeignKey,
    NotNull,
    Check,
}

impl ConstraintKind {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unique => "unique_violation",
            Self::Exclusion => "exclusion_violation",
            Self::ForeignKey => "foreign_key_violation",
            Self::NotNull => "not_null_violation",
            Self::Check => "check_violation",
        }
    }

    #[must_use]
    pub const fn is_conflict(self) -> bool {
        matches!(self, Self::Unique | Self::Exclusion | Self::ForeignKey)
    }
}

impl fmt::Display for ConstraintKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// The single repository error type of the workspace.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RepositoryError {
    #[error("{entity} not found{}", key_suffix(.key.as_deref()))]
    NotFound {
        entity: &'static str,
        key: Option<String>,
    },

    #[error("{entity} {key} conflicts: {reason}")]
    Conflict {
        entity: &'static str,
        key: String,
        reason: String,
    },

    #[error("{kind} on constraint {constraint}")]
    Constraint {
        kind: ConstraintKind,
        constraint: String,
        #[source]
        source: BoxedSource,
    },

    #[error("invalid argument {field}: {reason}")]
    InvalidArgument { field: &'static str, reason: String },

    #[error("invalid stored data in {field}: {reason}")]
    InvalidData { field: &'static str, reason: String },

    #[error("could not decode {context}")]
    Decode {
        context: String,
        #[source]
        source: BoxedSource,
    },

    #[error("database error: {source}")]
    Database {
        sqlstate: Option<String>,
        #[source]
        source: BoxedSource,
    },

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("SQL could not be split into statements: {0}")]
    SqlParse(#[source] BoxedSource),

    #[error("Failed to establish database connection")]
    Connection(#[source] Box<Self>),

    #[error("Failed to execute SQL statement: {statement}")]
    Statement {
        statement: String,
        #[source]
        source: Box<Self>,
    },

    #[error("Failed to read SQL file {path}")]
    SqlFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("internal repository error: {0}")]
    Internal(String),
}

impl RepositoryError {
    pub fn not_found(entity: &'static str, key: impl fmt::Display) -> Self {
        Self::NotFound {
            entity,
            key: Some(key.to_string()),
        }
    }

    pub fn conflict(
        entity: &'static str,
        key: impl fmt::Display,
        reason: impl Into<String>,
    ) -> Self {
        Self::Conflict {
            entity,
            key: key.to_string(),
            reason: reason.into(),
        }
    }

    pub fn invalid_argument(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidArgument {
            field,
            reason: reason.into(),
        }
    }

    pub fn invalid_data(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidData {
            field,
            reason: reason.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn decode(context: impl Into<String>, source: impl Into<BoxedSource>) -> Self {
        Self::Decode {
            context: context.into(),
            source: source.into(),
        }
    }

    pub fn database(source: impl Into<BoxedSource>) -> Self {
        let source: BoxedSource = source.into();
        #[cfg(feature = "sqlx")]
        let source = match source.downcast::<sqlx::Error>() {
            Ok(sqlx_error) => return Self::from(*sqlx_error),
            Err(other) => other,
        };
        Self::Database {
            sqlstate: None,
            source,
        }
    }

    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        match self {
            Self::Database { sqlstate, .. } => sqlstate.as_deref(),
            Self::Statement { source, .. } | Self::Connection(source) => source.sqlstate(),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }

    #[must_use]
    pub const fn is_constraint(&self) -> bool {
        matches!(self, Self::Constraint { .. })
    }

    #[must_use]
    pub const fn is_conflict(&self) -> bool {
        match self {
            Self::Conflict { .. } => true,
            Self::Constraint { kind, .. } => kind.is_conflict(),
            _ => false,
        }
    }

    #[must_use]
    pub fn is_serialization_failure(&self) -> bool {
        // Why: Postgres aborts one side of a serialization conflict (40001) or a
        // deadlock (40P01) and documents both as "retry the transaction".
        matches!(self.sqlstate(), Some("40001" | "40P01"))
    }

    #[must_use]
    pub fn is_invalid_function_definition(&self) -> bool {
        // Why: Postgres refuses CREATE OR REPLACE FUNCTION that changes the return
        // type or parameter names of an existing function (42P13); only DROP then
        // CREATE reshapes it.
        self.sqlstate() == Some("42P13")
    }
}

fn key_suffix(key: Option<&str>) -> String {
    key.map(|key| format!(": {key}")).unwrap_or_default()
}
