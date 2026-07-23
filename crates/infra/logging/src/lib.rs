//! # systemprompt-logging
//!
//! Tracing and audit infrastructure for systemprompt.io. Owns the
//! structured-event pipeline, the database-backed `tracing` layer,
//! log/analytics repositories, retention scheduling, and a typed query surface
//! over the audit trail (traces, AI requests, MCP tool executions).
//!
//! ## Feature flags
//!
//! | Feature   | Description                                                               |
//! |-----------|---------------------------------------------------------------------------|
//! | (default) | Database layer, repositories, trace queries, retention scheduler          |
//! | `cli`     | CLI display helpers (`CliService`, tables, banners) — pulls in `console`, `indicatif` |
//!
//! ## Top-level entry points
//!
//! - [`init_logging`] / [`init_console_logging`] /
//!   [`init_console_logging_with_level`] — install the global `tracing`
//!   subscriber (with optional database sink).
//! - [`LoggingExtension`] — schema/extension registration via the `inventory`
//!   framework.
//! - [`LoggingRepository`], [`AnalyticsRepository`] — direct repository access.
//! - [`TraceQueryService`], [`AiTraceService`] — typed audit/trace queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod attribution;
pub mod extension;
pub mod layer;
pub mod models;
pub mod repository;
mod sanitize;
pub mod services;
pub mod trace;

pub use attribution::{LogAttributionUnset, install_log_attribution, platform_attribution};
pub use extension::LoggingExtension;

pub use layer::{DatabaseLayer, enqueue_background};
pub use models::{LogActor, LogEntry, LogFilter, LogLevel};
pub use repository::{AnalyticsEvent, AnalyticsRepository, LoggingRepository};
#[cfg(feature = "cli")]
pub use services::CliService;
pub use services::{
    BufferedNotice, DatabaseLogService, FilterSystemFields, LogThrottle, LoggingMaintenanceService,
    RequestSpan, RequestSpanBuilder, SystemSpan, buffer_notice, drain_notices, is_startup_mode,
    is_structured_output, mark_structured_emitted, publish_log, set_log_publisher,
    set_startup_mode, set_structured_output, structured_was_emitted,
};
pub use trace::{
    AiRequestDetail, AiRequestFilter, AiRequestInfo, AiRequestListItem, AiRequestStats,
    AiRequestSummary, AiTraceService, AuditLookupResult, AuditToolCallRow, ConversationMessage,
    ExecutionStep, ExecutionStepSummary, LevelCount, LinkedMcpCall, LogSearchFilter, LogSearchItem,
    LogTimeRange, McpExecutionSummary, McpToolExecution, ModelStatsRow, ModuleCount,
    ProviderStatsRow, TaskArtifact, TaskInfo, ToolExecutionFilter, ToolExecutionItem, ToolLogEntry,
    TraceEvent, TraceListFilter, TraceListItem, TraceQueryService,
};

use std::sync::OnceLock;

use layer::ProxyDatabaseLayer;
use systemprompt_database::DbPool;
use tracing::Level;
use tracing_subscriber::filter::FilterFn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

static SUBSCRIBER_INITIALIZED: OnceLock<()> = OnceLock::new();
static DB_PROXY: OnceLock<ProxyDatabaseLayer> = OnceLock::new();

const NOISE_FILTERS: &[&str] = &[
    "tokio_cron_scheduler=warn",
    "sqlx::postgres::notice=warn",
    "sqlx::query=warn",
    "handlebars=warn",
    "systemprompt_database::lifecycle=info",
    "systemprompt_templates=info",
    "systemprompt_extension::registry=info",
    "systemprompt_api::services::middleware::session=info",
    "rmcp=warn",
    "rmcp::transport=warn",
];

fn build_filter(base: &str) -> EnvFilter {
    let filter_str = std::iter::once(base.to_owned())
        .chain(NOISE_FILTERS.iter().map(ToString::to_string))
        .collect::<Vec<_>>()
        .join(",");
    EnvFilter::new(filter_str)
}

// Why: Installs the global subscriber, idempotently: the first call wins and
// later calls no-op. This is load-bearing, not defensive — startup installs the
// console subscriber before the database pool exists, then `init_logging`
// re-enters here to guarantee the subscriber is present before attaching the
// DB sink.
fn ensure_subscriber(level_override: Option<&str>) {
    if SUBSCRIBER_INITIALIZED.set(()).is_err() {
        return;
    }

    let base_filter = level_override.map_or_else(
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| build_filter("info")),
        |level| EnvFilter::try_from_default_env().unwrap_or_else(|_| build_filter(level)),
    );

    let gate_active = level_override.is_none();
    let startup_gate = FilterFn::new(move |meta| {
        !(gate_active && is_startup_mode()) || *meta.level() <= Level::WARN
    });

    let fmt_layer = tracing_subscriber::fmt::layer()
        .fmt_fields(FilterSystemFields::new())
        .with_target(true)
        .with_writer(std::io::stderr)
        .log_internal_errors(true)
        .with_filter(base_filter)
        .with_filter(startup_gate);

    let proxy = DB_PROXY.get_or_init(ProxyDatabaseLayer::new).clone();
    let db_layer = proxy.with_filter(build_filter("info"));

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(db_layer)
        .init();
}

pub fn init_logging(db_pool: DbPool) {
    ensure_subscriber(None);

    let proxy = DB_PROXY.get_or_init(ProxyDatabaseLayer::new);
    proxy.attach(db_pool);
}

pub fn init_console_logging() {
    init_console_logging_with_level(None);
}

pub fn init_console_logging_with_level(level: Option<&str>) {
    ensure_subscriber(level);
}
