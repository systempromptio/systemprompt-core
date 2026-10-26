//! Logging services: the `tracing` write path and span helpers.
//!
//! [`DatabaseLogService`] persists structured log entries; [`output`] holds the
//! global publish path used during startup and steady state; [`spans`] provides
//! the request and system span builders. The CLI display sink (`CliService`)
//! is gated behind the `cli` feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
pub mod cli;
mod database_log;
mod format;
mod json_format;
mod maintenance;
pub mod output;
pub mod spans;
mod throttle;

#[cfg(feature = "cli")]
pub use cli::CliService;
pub use database_log::DatabaseLogService;
pub use format::FilterSystemFields;
pub use json_format::{ConsoleEventFormat, JsonEventFormat};
pub use maintenance::LoggingMaintenanceService;
pub use output::{BufferedNotice, OutputState};
pub use spans::{SYSTEM_LOG_OWNER, SystemSpan};
pub use throttle::LogThrottle;
