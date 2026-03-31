pub mod extension;
pub mod layer;
pub mod models;
pub mod repository;
pub mod services;
pub mod trace;

pub use extension::LoggingExtension;

pub use layer::DatabaseLayer;
pub use models::{LogEntry, LogFilter, LogLevel};
pub use repository::{AnalyticsEvent, AnalyticsRepository, LoggingRepository};
#[cfg(feature = "cli")]
pub use services::CliService;
pub use services::{
    DatabaseLogService, FilterSystemFields, LoggingMaintenanceService, RequestSpan,
    RequestSpanBuilder, SystemSpan, is_startup_mode, publish_log, set_log_publisher,
    set_startup_mode,
};
pub use trace::{
    AiRequestDetail, AiRequestFilter, AiRequestInfo, AiRequestListItem, AiRequestStats,
    AiRequestSummary, AiTraceService, AuditLookupResult, AuditToolCallRow, ConversationMessage,
    ExecutionStep, ExecutionStepSummary, LinkedMcpCall, LogSearchFilter, LogSearchItem,
    McpExecutionSummary, McpToolExecution, ModelStatsRow, ProviderStatsRow, TaskArtifact, TaskInfo,
    ToolExecutionFilter, ToolExecutionItem, ToolLogEntry, TraceEvent, TraceListFilter,
    TraceListItem, TraceQueryService,
};

use std::sync::OnceLock;

use systemprompt_database::DbPool;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

static LOGGING_INITIALIZED: OnceLock<()> = OnceLock::new();

const NOISE_FILTERS: &[&str] = &[
    "tokio_cron_scheduler=warn",
    "sqlx::postgres::notice=warn",
    "sqlx::query=warn",
    "handlebars=warn",
    "systemprompt_database::lifecycle=info",
    "systemprompt_templates=info",
    "systemprompt_extension::registry=info",
    "systemprompt_api::services::middleware::session=info",
];

fn build_filter(base: &str) -> EnvFilter {
    let filter_str = std::iter::once(base.to_string())
        .chain(NOISE_FILTERS.iter().map(ToString::to_string))
        .collect::<Vec<_>>()
        .join(",");
    EnvFilter::new(filter_str)
}

pub fn init_logging(db_pool: DbPool) {
    if LOGGING_INITIALIZED.set(()).is_err() {
        return;
    }

    let console_filter = if is_startup_mode() {
        EnvFilter::new("warn")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| build_filter("info"))
    };

    let fmt_layer = tracing_subscriber::fmt::layer()
        .fmt_fields(FilterSystemFields::new())
        .with_target(true)
        .with_writer(std::io::stderr)
        .with_filter(console_filter);

    let db_layer =
        DatabaseLayer::new(db_pool).with_filter(tracing_subscriber::filter::LevelFilter::INFO);

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(db_layer)
        .init();
}

pub fn init_console_logging() {
    init_console_logging_with_level(None);
}

pub fn init_console_logging_with_level(level: Option<&str>) {
    if LOGGING_INITIALIZED.set(()).is_err() {
        return;
    }

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| build_filter(level.unwrap_or("info")));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}
