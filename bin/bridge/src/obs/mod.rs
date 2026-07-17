//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod output {
    use crate::auth::types::HelperOutput;
    use std::io::Write;

    pub fn emit(output: &HelperOutput) -> std::io::Result<()> {
        let json = serde_json::to_string(output)?;
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(json.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()
    }

    pub fn diag(msg: &str) {
        tracing::warn!(target: "systemprompt_bridge", "{msg}");
    }
}

pub use tracing_init::{init, install_panic_hook, log_dir, log_file_path};

mod format;

pub mod tracing_init {
    use std::path::PathBuf;
    use std::sync::{Once, OnceLock};

    use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::EnvFilter;

    use super::format::{BridgeFormat, TeeWriter};

    static INIT: Once = Once::new();
    static GUARD: OnceLock<WorkerGuard> = OnceLock::new();
    pub(super) static FILE_WRITER: OnceLock<NonBlocking> = OnceLock::new();

    fn json_format_requested() -> bool {
        std::env::var(crate::brand::brand().env("LOG_FORMAT"))
            .is_ok_and(|v| v.eq_ignore_ascii_case("json"))
    }

    pub fn init() {
        INIT.call_once(|| {
            install_file_writer();
            let filter = EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,systemprompt_bridge::proxy=debug"));
            if json_format_requested() {
                _ = tracing_subscriber::fmt()
                    .with_writer(TeeWriter)
                    .with_env_filter(filter)
                    .json()
                    .flatten_event(true)
                    .try_init();
            } else {
                _ = tracing_subscriber::fmt()
                    .with_writer(TeeWriter)
                    .with_env_filter(filter)
                    .event_format(BridgeFormat)
                    .try_init();
            }
            tracing::info!(
                "log dir: {}",
                log_dir().map_or_else(|| "<disabled>".to_owned(), |p| p.display().to_string())
            );
        });
    }

    fn install_file_writer() {
        let Some(dir) = log_dir() else {
            return;
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            #[expect(
                clippy::print_stderr,
                reason = "tracing subscriber not yet installed; stderr is the only diagnostic \
                          channel"
            )]
            {
                eprintln!(
                    "[{}] cannot create log dir {}: {e}",
                    crate::brand::brand().binary_name,
                    dir.display()
                );
            }
            return;
        }
        let appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("bridge")
            .filename_suffix("log")
            .max_log_files(7)
            .build(&dir);
        let appender = match appender {
            Ok(a) => a,
            Err(e) => {
                #[expect(
                    clippy::print_stderr,
                    reason = "tracing subscriber not yet installed; stderr is the only diagnostic \
                              channel"
                )]
                {
                    eprintln!(
                        "[{}] rolling appender init failed: {e}",
                        crate::brand::brand().binary_name
                    );
                }
                return;
            },
        };
        let (writer, guard) = tracing_appender::non_blocking(appender);
        _ = GUARD.set(guard);
        _ = FILE_WRITER.set(writer);
    }

    pub fn log_dir() -> Option<PathBuf> {
        platform_log_dir()
    }

    pub fn log_file_path() -> Option<PathBuf> {
        let dir = log_dir()?;
        let day = chrono::Utc::now().format("%Y-%m-%d");
        Some(dir.join(format!("bridge.log.{day}")))
    }

    #[cfg(target_os = "windows")]
    fn platform_log_dir() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA").map(|p| {
            PathBuf::from(p)
                .join("Claude")
                .join(crate::brand::brand().working_dir_name)
        })
    }

    #[cfg(target_os = "macos")]
    fn platform_log_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| {
            h.join("Library")
                .join("Logs")
                .join(crate::brand::brand().working_dir_name)
        })
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn platform_log_dir() -> Option<PathBuf> {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("state")))
            .map(|base| base.join(crate::brand::brand().working_dir_name))
    }

    // Install before `init` so panics during subscriber setup are captured.
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
                _ = std::fs::create_dir_all(&dir);
                let path = dir.join(format!("bridge-crash-{ts}.log"));
                _ = std::fs::write(&path, &dump);
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
