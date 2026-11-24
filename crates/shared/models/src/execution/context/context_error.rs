//! Request-context construction error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use systemprompt_identifiers::{TaskId, UserId};
use systemprompt_traits::BoxedSource;
use thiserror::Error;

#[derive(Debug, Clone)]
pub enum ContextIdSource {
    Direct(String),
    FromTask { task_id: TaskId },
}

#[derive(Debug, Error)]
pub enum ContextExtractionError {
    #[error("Missing required header: {header}")]
    MissingHeader { header: String },

    #[error("Missing Authorization header")]
    MissingAuthHeader,

    #[error("Invalid JWT token: {0}")]
    InvalidToken(#[source] BoxedSource),

    #[error("Token revoked")]
    Revoked,

    #[error("JWT missing required 'session_id' claim")]
    MissingSessionId,

    #[error("JWT missing required 'sub' (user_id) claim")]
    MissingUserId,

    #[error(
        "Missing required 'x-context-id' header (for MCP routes) or contextId in body (for A2A \
         routes)"
    )]
    MissingContextId,

    #[error("Invalid header value: {header}, reason: {reason}")]
    InvalidHeaderValue { header: String, reason: String },

    #[error("Database error: {context}: {source}")]
    DatabaseError {
        context: String,
        #[source]
        source: BoxedSource,
    },

    #[error("User not found: User {user_id} {state}")]
    UserNotFound {
        user_id: UserId,
        state: UserUnavailable,
    },

    #[error("Forbidden header '{header}': {reason}")]
    ForbiddenHeader { header: String, reason: String },

    #[error("Request body exceeds {limit} bytes")]
    PayloadTooLarge { limit: usize },
}

/// Why an authenticated user can no longer act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserUnavailable {
    Missing,
    Inactive,
}

impl fmt::Display for UserUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Missing => "no longer exists",
            Self::Inactive => "is not active",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RequestContextValidationError {
    #[error("Missing task_id for task execution")]
    MissingTaskId,

    #[error("Missing authentication token")]
    MissingAuthToken,

    #[error("User is not authenticated")]
    Unauthenticated,
}
