//! Typed error hierarchy for the `systemprompt-agent` crate.
//!
//! Public APIs return concrete `thiserror`-derived enums instead of
//! `anyhow::Error` so that downstream callers can match on error variants
//! without string parsing.

use systemprompt_identifiers::TaskId;
use thiserror::Error;

/// Errors raised while parsing structured rows from the database into domain
/// types.
#[derive(Debug, Error)]
pub enum RowParseError {
    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid datetime for field '{field}'")]
    InvalidDatetime { field: String },

    #[error("JSON parse error for field '{field}': {source}")]
    JsonParse {
        field: String,
        #[source]
        source: serde_json::Error,
    },
}

/// Errors raised while reading or mutating tasks in the agent repository.
#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Task UUID missing from database row")]
    MissingTaskUuid,

    #[error("Agent name not found for task {task_id}")]
    MissingAgentName { task_id: TaskId },

    #[error("Context ID missing from database row")]
    MissingContextId,

    #[error("Invalid task state: {state}")]
    InvalidTaskState { state: String },

    #[error(transparent)]
    RowParse(#[from] RowParseError),

    #[error("Metadata parse error: {0}")]
    InvalidMetadata(#[from] serde_json::Error),

    #[error("Empty task ID provided")]
    EmptyTaskId,

    #[error("Invalid task ID format: {id}")]
    InvalidTaskIdFormat { id: String },

    #[error("Message ID missing from database row")]
    MissingMessageId,

    #[error("Tool name missing for tool execution")]
    MissingToolName,

    #[error("Tool call ID missing for tool execution")]
    MissingCallId,

    #[error("Created timestamp missing from database")]
    MissingCreatedTimestamp,

    #[error("Database error: {0}")]
    Database(String),
}

/// Errors raised while reading or mutating conversational contexts.
#[derive(Debug, Error)]
pub enum ContextError {
    #[error("Context UUID missing from database row")]
    MissingUuid,

    #[error("Context name missing from database row")]
    MissingName,

    #[error("User ID missing from database row")]
    MissingUserId,

    #[error(transparent)]
    RowParse(#[from] RowParseError),

    #[error("Role serialization error: {0}")]
    RoleSerialization(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(String),
}

/// Errors raised while reading, storing, or transforming task artifacts.
#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("Artifact UUID missing from database row")]
    MissingUuid,

    #[error("Artifact type missing from database row")]
    MissingType,

    #[error("Context ID missing for artifact")]
    MissingContextId,

    #[error(transparent)]
    RowParse(#[from] RowParseError),

    #[error("Invalid tool response schema: expected {expected}, found keys: {actual_keys:?}")]
    InvalidSchema {
        expected: &'static str,
        actual_keys: Vec<String>,
        #[source]
        source: serde_json::Error,
    },

    #[error("Metadata parse error: {0}")]
    InvalidMetadata(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Transform error: {0}")]
    Transform(String),

    #[error("Metadata validation error: {0}")]
    MetadataValidation(String),
}

/// Errors raised while validating or parsing A2A JSON-RPC protocol payloads.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Tool name missing in tool call")]
    MissingToolName,

    #[error("Tool result error flag is required but was not provided")]
    MissingErrorFlag,

    #[error("Message ID missing")]
    MissingMessageId,

    #[error("Request ID missing")]
    MissingRequestId,

    #[error("Latency value missing or invalid")]
    InvalidLatency,

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(String),
}

/// Top-level error type aggregating every failure mode exposed by the agent
/// crate.
#[derive(Debug, Error)]
pub enum AgentError {
    /// Task-layer error from the repository or task constructors.
    #[error("Task error: {0}")]
    Task(#[from] TaskError),

    /// Context-layer error from the conversational context repository.
    #[error("Context error: {0}")]
    Context(#[from] ContextError),

    /// Artifact-layer error from the artifact repository or transformer.
    #[error("Artifact error: {0}")]
    Artifact(#[from] ArtifactError),

    /// A2A JSON-RPC protocol error.
    #[error("A2A protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    /// Generic repository error with a textual message.
    #[error("Repository error: {0}")]
    Repository(String),

    /// Generic database error with a textual message.
    #[error("Database error: {0}")]
    Database(String),

    /// Failure to acquire a connection pool or initialise a repository struct.
    #[error("repository init: {0}")]
    Init(String),

    /// Failure raised by the embedded A2A HTTP server (binding, serving,
    /// shutdown).
    #[error("server: {0}")]
    Server(String),

    /// Failure raised by an outbound webhook broadcast.
    #[error("webhook: {0}")]
    Webhook(String),

    /// Failure to load the global crate configuration.
    #[error("config: {0}")]
    Config(String),

    /// HTTP transport error from `reqwest`.
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// Agent could not be located by name.
    #[error("agent not found: {0}")]
    NotFound(String),

    /// Agent process failed to spawn.
    #[error("spawn failed: {0}")]
    Spawn(String),

    /// Lifecycle (start/stop/reload) error.
    #[error("lifecycle: {0}")]
    Lifecycle(String),

    /// Input validation failure.
    #[error("validation: {0}")]
    Validation(String),

    /// Underlying `sqlx` driver error.
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    /// Underlying `std::io` error.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Catch-all for upstream `anyhow::Error` values that are not yet typed.
    ///
    /// New code should prefer a dedicated variant; this exists to compose with
    /// crates whose public API still returns `anyhow::Error`.
    #[error(transparent)]
    Other(#[from] anyhow::Error),

    /// Failure loading service configuration via `systemprompt-loader`.
    #[error("services config: {0}")]
    ServicesConfig(#[from] systemprompt_loader::ConfigLoadError),
}

/// Convenience `Result` alias parameterised over [`AgentError`].
pub type AgentResult<T> = Result<T, AgentError>;

impl From<AgentError> for systemprompt_traits::RepositoryError {
    fn from(err: AgentError) -> Self {
        match err {
            AgentError::Sqlx(e) => Self::Database(Box::new(e)),
            other => Self::Database(other.to_string().into()),
        }
    }
}
