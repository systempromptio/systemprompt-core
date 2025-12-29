pub mod cli;
mod database_log;
mod maintenance;
pub mod output;
pub mod retention;
pub mod spans;

pub use cli::CliService;
pub use database_log::DatabaseLogService;
pub use maintenance::LoggingMaintenanceService;
pub use output::{
    get_log_publisher, get_output_mode, init_tui_mode, is_console_output_enabled, is_startup_mode,
    publish_log, set_log_publisher, set_output_mode, set_startup_mode, OutputMode,
};
pub use retention::{RetentionConfig, RetentionPolicy, RetentionScheduler};
pub use spans::{RequestSpan, RequestSpanBuilder, SystemSpan};
