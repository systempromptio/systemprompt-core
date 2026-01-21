use thiserror::Error;

pub const TASK_BASED_CONTEXT_MARKER: &str = "__task_based__";

#[derive(Debug, Clone)]
pub enum ContextIdSource {
    Direct(String),
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
