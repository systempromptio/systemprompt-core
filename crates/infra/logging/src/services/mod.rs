//! Logging services: the `tracing` write path, retention, and span helpers.
//!
//! [`DatabaseLogService`] persists structured log entries; [`output`] holds the
//! global publish path used during startup and steady state; [`retention`]
//! enforces age-based cleanup on a schedule; [`spans`] provides the request and
//! system span builders. The CLI display sink ([`CliService`]) is gated behind
//! the `cli` feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
pub mod cli;
mod database_log;
mod format;
mod maintenance;
pub mod output;
pub mod retention;
pub mod spans;
mod throttle;

#[cfg(feature = "cli")]
pub use cli::CliService;
pub use database_log::DatabaseLogService;
pub use format::FilterSystemFields;
pub use maintenance::LoggingMaintenanceService;
pub use output::{
    BufferedNotice, buffer_notice, drain_notices, get_log_publisher, is_startup_mode,
    is_structured_output, mark_structured_emitted, publish_log, set_log_publisher,
    set_startup_mode, set_structured_output, structured_was_emitted,
};
pub use retention::{RetentionConfig, RetentionPolicy, RetentionScheduler};
pub use spans::{RequestSpan, RequestSpanBuilder, SystemSpan};
pub use throttle::LogThrottle;
