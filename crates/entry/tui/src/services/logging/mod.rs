use std::fs;
use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_file_logging() -> Option<WorkerGuard> {
    let log_dir = get_log_directory();

    if fs::create_dir_all(&log_dir).is_err() {
        return None;
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "tui.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,systemprompt=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true),
        )
        .init();

    Some(guard)
}

fn get_log_directory() -> PathBuf {
    if let Ok(log_dir) = std::env::var("SYSTEMPROMPT_LOG_DIR") {
        return PathBuf::from(log_dir);
    }

    if let Ok(services_path) = std::env::var("SYSTEMPROMPT_SERVICES_PATH") {
        let services = PathBuf::from(services_path);
        if let Some(parent) = services.parent() {
            return parent.join("logs");
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("logs")
}

pub fn get_log_file_path() -> PathBuf {
    get_log_directory().join("tui.log")
}

pub use tracing::{debug, error, info, trace, warn};
