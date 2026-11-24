//! Semantic validation errors raised by the services / agents / plugins
//! / hooks / modules validation passes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::auth::{Permission, permissions_to_string};

/// The configuration entry a validation message is about, rendered as the
/// lead-in of that message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationSubject {
    Named { kind: &'static str, name: String },
    Clause { kind: &'static str, name: String },
    Scope(String),
    Label(String),
    Global,
}

impl ValidationSubject {
    #[must_use]
    pub fn named(kind: &'static str, name: impl fmt::Display) -> Self {
        Self::Named {
            kind,
            name: name.to_string(),
        }
    }

    #[must_use]
    pub fn clause(kind: &'static str, name: impl fmt::Display) -> Self {
        Self::Clause {
            kind,
            name: name.to_string(),
        }
    }

    #[must_use]
    pub fn scope(scope: impl Into<String>) -> Self {
        Self::Scope(scope.into())
    }

    #[must_use]
    pub fn label(label: impl Into<String>) -> Self {
        Self::Label(label.into())
    }
}

impl fmt::Display for ValidationSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named { kind, name } => write!(f, "{kind} '{name}': "),
            Self::Clause { kind, name } => write!(f, "{kind} '{name}' "),
            Self::Scope(scope) => write!(f, "{scope}: "),
            Self::Label(label) => write!(f, "{label} "),
            Self::Global => Ok(()),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ServicesValidationError {
    #[error("{subject}{reason}")]
    Required {
        subject: ValidationSubject,
        reason: String,
    },

    #[error("{subject}{reason}")]
    InvalidField {
        subject: ValidationSubject,
        reason: String,
    },

    #[error("{context}: {source}")]
    InvalidFieldCause {
        context: String,
        #[source]
        source: Arc<dyn Error + Send + Sync>,
    },

    #[error(
        "Port conflict: {port} used by both {first_kind} '{first_name}' and {second_kind} \
         '{second_name}'"
    )]
    PortConflict {
        port: u16,
        first_kind: &'static str,
        first_name: String,
        second_kind: &'static str,
        second_name: String,
    },

    #[error("{subject}{reason}")]
    UnknownReference {
        subject: ValidationSubject,
        reason: String,
    },

    #[error("{}", chain.join(" → "))]
    CircularDependency { chain: Vec<String> },

    #[error("{subject}{reason}")]
    BusinessRule {
        subject: ValidationSubject,
        reason: String,
    },

    #[error("{owner}: {field} names the built-in secret `{secret}`; it must name a custom secret")]
    BuiltInSecretRef {
        owner: String,
        field: &'static str,
        secret: String,
    },

    #[error(
        "{owner}: oauth.scopes [{}] holds no scope an interactively signed-in user can carry, \
         so no non-admin user can reach it; require `user` (or `admin`, or `anonymous`) \
         alongside it, or set `oauth.service_only: true` if only client-credentials or hook \
         tokens should reach it",
        permissions_to_string(scopes)
    )]
    UnreachableByUsers {
        owner: String,
        scopes: Vec<Permission>,
    },

    #[error(
        "{owner}: oauth.service_only is set but oauth.scopes [{}] names the user-tier scope \
         `{scope}`; list only service scopes (`service`, `a2a`, `mcp`, `hook:govern`, \
         `hook:track`) or drop `service_only`",
        permissions_to_string(scopes)
    )]
    ServiceOnlyNamesUserScope {
        owner: String,
        scope: Permission,
        scopes: Vec<Permission>,
    },
}

impl ServicesValidationError {
    #[must_use]
    pub fn required(subject: ValidationSubject, reason: impl Into<String>) -> Self {
        Self::Required {
            subject,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn invalid_field(subject: ValidationSubject, reason: impl Into<String>) -> Self {
        Self::InvalidField {
            subject,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn invalid_field_cause(
        context: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::InvalidFieldCause {
            context: context.into(),
            source: Arc::new(source),
        }
    }

    #[must_use]
    pub fn port_conflict(
        port: u16,
        first: (&'static str, &str),
        second: (&'static str, &str),
    ) -> Self {
        Self::PortConflict {
            port,
            first_kind: first.0,
            first_name: first.1.to_owned(),
            second_kind: second.0,
            second_name: second.1.to_owned(),
        }
    }

    #[must_use]
    pub fn unknown_reference(subject: ValidationSubject, reason: impl Into<String>) -> Self {
        Self::UnknownReference {
            subject,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn circular_dependency(chain: Vec<String>) -> Self {
        Self::CircularDependency { chain }
    }

    #[must_use]
    pub fn business_rule(subject: ValidationSubject, reason: impl Into<String>) -> Self {
        Self::BusinessRule {
            subject,
            reason: reason.into(),
        }
    }
}
