//! Trace-domain DTOs grouped by cohesion: trace listings + events, AI request
//! analytics, MCP/tool executions, and log-search summaries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai;
mod log;
mod tool;
mod trace;

pub use ai::{
    AiRequestClientEvidence, AiRequestDetail, AiRequestFilter, AiRequestInfo, AiRequestListItem,
    AiRequestStats, AuditPage, ConversationMessage, ModelStatsRow, ProviderStatsRow, RequestCursor,
    RequestCursorError,
};
pub use log::{LevelCount, LogSearchFilter, LogSearchItem, LogTimeRange, ModuleCount};
pub use tool::{
    AuditLookupResult, AuditToolCallRow, LinkedMcpCall, McpToolExecution, TaskArtifact,
    ToolExecutionFilter, ToolExecutionItem, ToolLogEntry,
};
pub use trace::{
    AiRequestSummary, ExecutionStep, ExecutionStepSummary, McpExecutionSummary, TaskInfo,
    TraceEvent, TraceListFilter, TraceListItem,
};
