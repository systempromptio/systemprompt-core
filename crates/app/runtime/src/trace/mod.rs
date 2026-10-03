//! Trace query services backing the `logs` CLI and audit surfaces.
//!
//! [`TraceQueryService`] reconstructs a request's timeline — log events, AI
//! requests, MCP executions, and execution steps — from a trace id, while
//! [`AiTraceService`] resolves per-task execution steps. Both read through
//! [`TraceRepository`]; result shapes are re-exported from `models`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_trace_service;
mod models;
mod repository;
mod service;

pub use ai_trace_service::AiTraceService;
pub use models::{
    AiRequestClientEvidence, AiRequestDetail, AiRequestFilter, AiRequestInfo, AiRequestListItem,
    AiRequestStats, AiRequestSummary, AuditLookupResult, AuditPage, AuditToolCallRow,
    ConversationMessage, ExecutionStep, ExecutionStepSummary, LevelCount, LinkedMcpCall,
    LogSearchFilter, LogSearchItem, LogTimeRange, McpExecutionSummary, McpToolExecution,
    ModelStatsRow, ModuleCount, ProviderStatsRow, RequestCursor, RequestCursorError, TaskArtifact,
    TaskInfo, ToolExecutionFilter, ToolExecutionItem, ToolLogEntry, TraceEvent, TraceListFilter,
    TraceListItem,
};
pub use repository::TraceRepository;
pub use service::TraceQueryService;

/// Why a trace lookup could not be answered.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("Database operation failed")]
    Database(#[from] sqlx::Error),

    #[error("Task not found: {partial_id}")]
    TaskNotFound { partial_id: String },

    #[error("Stored identifier is malformed")]
    MalformedId(#[from] systemprompt_identifiers::error::IdValidationError),

    #[error("Stored log level is malformed")]
    MalformedLevel(#[from] systemprompt_logging::models::LoggingError),
}
