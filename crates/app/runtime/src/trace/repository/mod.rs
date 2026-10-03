//! Read-only SQL over the tracing and audit tables.
//!
//! [`TraceRepository`] owns every query the trace services issue; each
//! submodule adds one group of methods (log events, AI requests, audit views,
//! MCP executions, execution steps, log browsing) to the same type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_trace;
mod audit;
mod events;
mod list;
mod log_lookup;
mod log_search;
mod log_summary;
mod mcp_trace;
mod request;
mod request_stats;
mod step;
mod tool;

use sqlx::PgPool;
use std::sync::Arc;

use super::TraceError;

pub(super) type Result<T> = std::result::Result<T, TraceError>;

/// Query access to the trace, log and audit tables.
#[derive(Debug, Clone)]
pub struct TraceRepository {
    pool: Arc<PgPool>,
}

impl TraceRepository {
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}
