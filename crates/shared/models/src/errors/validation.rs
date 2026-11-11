//! Semantic validation errors raised by the services / agents / plugins
//! / hooks / modules validation passes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error;

use crate::auth::{Permission, permissions_to_string};
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ServicesValidationError {
    #[error("{0}")]
    Required(String),

    #[error("{0}")]
    InvalidField(String),

    #[error("{context}: {source}")]
    InvalidFieldCause {
        context: String,
        #[source]
        source: Arc<dyn Error + Send + Sync>,
    },

    #[error("{0}")]
    PortConflict(String),

    #[error("{0}")]
    UnknownReference(String),

    #[error("{0}")]
    CircularDependency(String),

    #[error("{0}")]
    BusinessRule(String),

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
    pub fn required(msg: impl Into<String>) -> Self {
        Self::Required(msg.into())
    }

    #[must_use]
    pub fn invalid_field(msg: impl Into<String>) -> Self {
        Self::InvalidField(msg.into())
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
    pub fn port_conflict(msg: impl Into<String>) -> Self {
        Self::PortConflict(msg.into())
    }

    #[must_use]
    pub fn unknown_reference(msg: impl Into<String>) -> Self {
        Self::UnknownReference(msg.into())
    }

    #[must_use]
    pub fn circular_dependency(msg: impl Into<String>) -> Self {
        Self::CircularDependency(msg.into())
    }

    #[must_use]
    pub fn business_rule(msg: impl Into<String>) -> Self {
        Self::BusinessRule(msg.into())
    }
}
