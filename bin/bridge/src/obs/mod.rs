//! Tracing initialisation for the bridge (console/file/JSON).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use tracing_init::{init, install_panic_hook, log_dir, log_file_path, logging_fault};

mod format;

/// A start-up step that failed without stopping the process.
///
/// Start-up touches metadata the process can live without: the recorded
/// proxy port, the cached MCP registry, the activity log, the log file. A
/// fault in any of them must reach `doctor` and the GUI, but must not brick
/// the very commands that repair it; a corrupt port file that makes
/// `doctor` exit 70 leaves nobody able to see what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupFault {
    pub component: &'static str,
    pub error: String,
}

impl StartupFault {
    #[must_use]
    pub fn new(component: &'static str, error: impl std::fmt::Display) -> Self {
        Self {
            component,
            error: error.to_string(),
        }
    }
}

impl std::fmt::Display for StartupFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.component, self.error)
    }
}

pub mod tracing_init {
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::EnvFilter;

    use super::format::{BridgeFormat, TeeWriter};

    // Why: the log file is diagnostics, not a dependency. A missing home or
    // an unwritable log directory must not stop `doctor` from running;
    // stderr still gets every WARN and the fault travels with the
    // initialisation outcome so `logging_fault` can report it.
    static INIT: OnceLock<Result<Option<String>, String>> = OnceLock::new();
    static GUARD: OnceLock<WorkerGuard> = OnceLock::new();
    pub(super) static FILE_WRITER: OnceLock<NonBlocking> = OnceLock::new();

    fn json_format_requested() -> bool {
        std::env::var(crate::brand::brand().env("LOG_FORMAT"))
            .is_ok_and(|v| v.eq_ignore_ascii_case("json"))
    }

    pub fn init() -> Result<(), String> {
        INIT.get_or_init(|| {
            let file_fault = install_file_writer().err();
            let filter = match EnvFilter::try_from_default_env() {
                Ok(filter) => filter,
                Err(e) if std::env::var_os("RUST_LOG").is_some() => {
                    return Err(format!("RUST_LOG: {e}"));
                },
                Err(_) => EnvFilter::new("info,systemprompt_bridge::proxy=debug"),
            };
            if json_format_requested() {
                tracing_subscriber::fmt()
                    .with_writer(TeeWriter)
                    .with_env_filter(filter)
                    .json()
                    .flatten_event(true)
                    .try_init()
                    .map_err(|e| format!("initialize tracing: {e}"))?;
            } else {
                tracing_subscriber::fmt()
                    .with_writer(TeeWriter)
                    .with_env_filter(filter)
                    .event_format(BridgeFormat)
                    .try_init()
                    .map_err(|e| format!("initialize tracing: {e}"))?;
            }
            Ok(file_fault)
        })
        .clone()
        .map(|_fault| ())
    }

    fn install_file_writer() -> Result<(), String> {
        let dir = log_dir().ok_or_else(|| "cannot resolve bridge log directory".to_owned())?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("create log directory {}: {e}", dir.display()))?;
        let appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("bridge")
            .filename_suffix("log")
            .max_log_files(7)
            .build(&dir)
            .map_err(|e| format!("open log directory {}: {e}", dir.display()))?;
        // Why: the log is the evidence for every failed subcommand; dropping
        // lines under back-pressure would lose exactly the burst that matters.
        let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
            .lossy(false)
            .finish(appender);
        GUARD
            .set(guard)
            .map_err(|_guard| "logging worker already installed".to_owned())?;
        FILE_WRITER
            .set(writer)
            .map_err(|_writer| "logging writer already installed".to_owned())?;
        Ok(())
    }

    pub fn logging_fault() -> Option<String> {
        INIT.get().and_then(|init| init.as_ref().ok()?.clone())
    }

    pub fn log_dir() -> Option<PathBuf> {
        platform_log_dir()
    }

    pub fn log_file_path() -> Option<PathBuf> {
        let dir = log_dir()?;
        let day = chrono::Utc::now().format("%Y-%m-%d");
        Some(dir.join(format!("bridge.log.{day}")))
    }

    fn platform_log_dir() -> Option<PathBuf> {
        log_base().map(|base| base.join(crate::brand::brand().working_dir_name))
    }

    fn log_base() -> Option<PathBuf> {
        if let Some(base) = crate::basedirs::state_home_override() {
            return Some(base);
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Claude"))
        }
        #[cfg(target_os = "macos")]
        {
            crate::basedirs::home_dir().map(|h| h.join("Library").join("Logs"))
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            crate::basedirs::home_dir().map(|h| h.join(".local").join("state"))
        }
    }

    pub fn install_panic_hook() {
        std::panic::set_hook(Box::new(|info| {
            let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
            let location = info.location().map_or_else(
                || "<unknown>".to_owned(),
                |l| format!("{}:{}", l.file(), l.line()),
            );
            let payload = info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .map(str::to_owned)
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_owned());
            let backtrace = backtrace::Backtrace::new();
            let dump =
                format!("panic at {location}\npayload: {payload}\n\nbacktrace:\n{backtrace:?}\n");
            if let Some(dir) = log_dir() {
                let path = dir.join(format!("bridge-crash-{ts}.log"));
                if let Err(e) = crate::fsutil::atomic_write_0600(&path, dump.as_bytes()) {
                    crate::stdio::eprint_str(&format!(
                        "cannot persist crash report {}: {e}",
                        path.display()
                    ));
                }
                tracing::error!(
                    crash_log = %path.display(),
                    location = %location,
                    payload = %payload,
                    "bridge panicked"
                );
            } else {
                tracing::error!(
                    location = %location,
                    payload = %payload,
                    "bridge panicked (no log dir available)"
                );
            }
            #[expect(
                clippy::print_stderr,
                reason = "panic hook last-resort dump: tracing may already be torn down at this \
                          point"
            )]
            {
                eprintln!("{dump}");
            }
        }));
    }
}
