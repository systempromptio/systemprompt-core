//! Logging domain types: entries, levels, filters, and errors.
//!
//! [`LogEntry`] is the structured record written to every sink; [`LogLevel`]
//! and [`LogActor`] classify it; [`LogFilter`] parameterizes paginated reads;
//! [`LogRow`] is the database projection. Failures surface as [`LoggingError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod log_entry;
mod log_error;
mod log_filter;
mod log_level;
mod log_row;

pub use log_entry::{LogActor, LogEntry};
pub use log_error::LoggingError;
pub use log_filter::LogFilter;
pub use log_level::LogLevel;
pub use log_row::LogRow;
