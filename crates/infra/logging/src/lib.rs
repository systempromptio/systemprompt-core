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
pub use services::{
    is_startup_mode, publish_log, set_log_publisher, set_startup_mode, CliService,
    DatabaseLogService, FilterSystemFields, LoggingMaintenanceService, RequestSpan,
    RequestSpanBuilder, SystemSpan,
};
pub use trace::{
    AiRequestInfo, AiRequestSummary, AiTraceService, ConversationMessage, ExecutionStep,
    ExecutionStepSummary, McpExecutionSummary, McpToolExecution, TaskArtifact, TaskInfo,
    ToolLogEntry, TraceEvent, TraceQueryService,
};

use std::sync::OnceLock;

use systemprompt_database::DbPool;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

static LOGGING_INITIALIZED: OnceLock<()> = OnceLock::new();

pub fn init_logging(db_pool: DbPool) {
    if LOGGING_INITIALIZED.set(()).is_err() {
        return;
    }

    let console_filter = if is_startup_mode() {
        EnvFilter::new("warn")
    } else {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,tokio_cron_scheduler=warn,sqlx::postgres::notice=warn"))
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
    if LOGGING_INITIALIZED.set(()).is_err() {
        return;
    }

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tokio_cron_scheduler=warn,sqlx::postgres::notice=warn"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}
