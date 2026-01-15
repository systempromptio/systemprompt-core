//! Context extraction error types and A2A protocol context handling.

use thiserror::Error;

/// Marker context ID for task-based methods where context is resolved from task
/// storage. Per A2A spec Section 7.3, tasks/get only requires task_id - the
/// context is stored with the task.
pub const TASK_BASED_CONTEXT_MARKER: &str = "__task_based__";

/// Result of context extraction from A2A payload.
/// Per A2A spec, message methods include contextId directly,
/// while task methods only have task ID (context resolved from storage).
#[derive(Debug, Clone)]
pub enum ContextIdSource {
    /// contextId found directly in payload (message/send, message/stream)
    Direct(String),
    /// Task-based method - context should be resolved from task storage
    FromTask { task_id: String },
}

#[derive(Debug, Error)]
pub enum ContextExtractionError {
    #[error("Missing required header: {0}")]
    MissingHeader(String),

    #[error("Missing Authorization header")]
    MissingAuthHeader,

    #[error("Invalid JWT token: {0}")]
    InvalidToken(String),

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

    #[error("Invalid user_id: {0}")]
    InvalidUserId(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Forbidden header '{header}': {reason}")]
    ForbiddenHeader { header: String, reason: String },
}
