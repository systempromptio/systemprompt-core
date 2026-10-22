//! # systemprompt-logging
//!
//! Tracing and audit infrastructure for systemprompt.io. Owns the
//! structured-event pipeline, the database-backed `tracing` layer,
//! log/analytics repositories, and the CLI display
//! sinks. The trace-assembly queries over agent, AI and MCP tables live in
//! `systemprompt_runtime::trace`.
//!
//! ## Feature flags
//!
//! | Feature   | Description                                                               |
//! |-----------|---------------------------------------------------------------------------|
//! | (default) | Database layer, repositories                                              |
//! | `cli`     | CLI display helpers (`CliService`, tables, banners) — pulls in `console`, `indicatif` |
//!
//! ## Top-level entry points
//!
//! - [`init_logging`] / [`init_console_logging`] /
//!   [`init_console_logging_with_level`] — install the global `tracing`
//!   subscriber (with optional database sink) and return its [`LogSink`].
//! - [`LoggingRuntime`] — the sink paired with the platform owner; built once
//!   at bootstrap and carried by `AppContext`.
//! - [`set_console_format`] — switch stderr between text and JSON lines
//!   (`observability.log_format`); both redact the same fields.
//! - [`LogSink::shutdown`] — flush the database sink's buffered and queued
//!   entries and stop its writer task; awaited on server shutdown.
//! - [`LoggingExtension`] — schema/extension registration via the `inventory`
//!   framework.
//! - [`LoggingRepository`], [`AnalyticsRepository`] — direct repository access.
//!
//! ## Process state
//!
//! The `tracing` subscriber is the one process singleton: `tracing` installs
//! its global dispatcher once, so the crate mirrors it with a set-once guard,
//! and the CLI output-mode flags ride with it. Everything else — the database
//! sink, the instance label, the drop count, the platform owner — is owned by
//! the [`LogSink`] / [`LoggingRuntime`] values. A sink is recovered from the
//! installed dispatcher rather than from a crate-level cell.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod extension;
pub mod layer;
pub mod models;
pub mod repository;
pub mod runtime;
pub mod sanitize;
pub mod services;

pub use extension::LoggingExtension;

pub use layer::{DatabaseLayer, LogWriterHandle, LogWriterShutdownError};
pub use models::{LogActor, LogEntry, LogFilter, LogLevel};
pub use repository::{
    AnalyticsEvent, AnalyticsRepository, LoggingOwnerReassignment, LoggingRepository,
};
pub use runtime::{LogSink, LoggingRuntime};
#[cfg(feature = "cli")]
pub use services::CliService;
pub use services::{
    BufferedNotice, ConsoleEventFormat, DatabaseLogService, FilterSystemFields, JsonEventFormat,
    LogThrottle, LoggingMaintenanceService, SYSTEM_LOG_OWNER, SystemSpan, buffer_notice,
    console_format, drain_notices, is_startup_mode, is_structured_output, mark_structured_emitted,
    reset_structured_emitted, set_console_format, set_startup_mode, set_structured_output,
    structured_was_emitted,
};

use std::sync::OnceLock;

use layer::{InstanceLabel, ProxyDatabaseLayer};
use systemprompt_database::DbPool;
use systemprompt_identifiers::InstanceId;
use tracing::Level;
use tracing_subscriber::filter::FilterFn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

static SUBSCRIBER_INITIALIZED: OnceLock<()> = OnceLock::new();

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

fn installed_proxy() -> Option<ProxyDatabaseLayer> {
    tracing::dispatcher::get_default(|dispatch| {
        dispatch.downcast_ref::<ProxyDatabaseLayer>().cloned()
    })
}

fn ensure_subscriber(level_override: Option<&str>) -> LogSink {
    if SUBSCRIBER_INITIALIZED.set(()).is_err() {
        return LogSink::from_proxy(installed_proxy().unwrap_or_default());
    }

    let proxy = ProxyDatabaseLayer::new();
    let instance: InstanceLabel = proxy.shared().instance.clone();

    let base_filter = level_override.map_or_else(
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| build_filter("info")),
        |level| EnvFilter::try_from_default_env().unwrap_or_else(|_| build_filter(level)),
    );

    let gate_active = level_override.is_none();
    let startup_gate = FilterFn::new(move |meta| {
        !(gate_active && is_startup_mode()) || *meta.level() <= Level::WARN
    });

    let fmt_layer = tracing_subscriber::fmt::layer()
        .fmt_fields(FilterSystemFields::labelled(instance.clone()))
        .event_format(ConsoleEventFormat::labelled(instance))
        .with_writer(std::io::stderr)
        .log_internal_errors(true)
        .with_filter(base_filter)
        .with_filter(startup_gate);

    let db_layer = proxy.clone().with_filter(build_filter("info"));

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(db_layer)
        .init();

    LogSink::from_proxy(proxy)
}

pub fn init_logging(db_pool: &DbPool, instance_id: Option<InstanceId>) -> LogSink {
    let sink = ensure_subscriber(None);
    sink.attach(db_pool, instance_id);
    sink
}

pub fn init_console_logging() -> LogSink {
    init_console_logging_with_level(None)
}

pub fn init_console_logging_with_level(level: Option<&str>) -> LogSink {
    ensure_subscriber(level)
}
