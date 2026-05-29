//! Logging services: the `tracing` write path, retention, and span helpers.
//!
//! [`DatabaseLogService`] persists structured log entries; [`output`] holds the
//! global publish path used during startup and steady state; [`retention`]
//! enforces age-based cleanup on a schedule; [`spans`] provides the request and
//! system span builders. The CLI display sink ([`CliService`]) is gated behind
//! the `cli` feature.

#[cfg(feature = "cli")]
pub mod cli;
mod database_log;
mod format;
mod maintenance;
pub mod output;
pub mod retention;
pub mod spans;

#[cfg(feature = "cli")]
pub use cli::CliService;
pub use database_log::DatabaseLogService;
pub use format::FilterSystemFields;
pub use maintenance::LoggingMaintenanceService;
pub use output::{
    get_log_publisher, is_startup_mode, publish_log, set_log_publisher, set_startup_mode,
};
pub use retention::{RetentionConfig, RetentionPolicy, RetentionScheduler};
pub use spans::{RequestSpan, RequestSpanBuilder, SystemSpan};
