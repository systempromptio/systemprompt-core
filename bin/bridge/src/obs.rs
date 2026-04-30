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

pub mod tracing_init {
    use std::fmt;
    use std::io::{self, Write};
    use std::path::PathBuf;
    use std::sync::{Once, OnceLock};

    use tracing::{Event, Subscriber};
    use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::field::Visit;
    use tracing_subscriber::fmt::format::Writer;
    use tracing_subscriber::fmt::{FormatEvent, FormatFields, MakeWriter};
    use tracing_subscriber::registry::LookupSpan;

    static INIT: Once = Once::new();
    static GUARD: OnceLock<WorkerGuard> = OnceLock::new();
    static FILE_WRITER: OnceLock<NonBlocking> = OnceLock::new();

    fn json_format_requested() -> bool {
        std::env::var("SP_BRIDGE_LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json"))
    }

    pub fn init() {
        INIT.call_once(|| {
            install_file_writer();
            let filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
            if json_format_requested() {
                let _ = tracing_subscriber::fmt()
                    .with_writer(TeeWriter)
                    .with_env_filter(filter)
                    .json()
                    .flatten_event(true)
                    .try_init();
            } else {
                let _ = tracing_subscriber::fmt()
                    .with_writer(TeeWriter)
                    .with_env_filter(filter)
                    .event_format(CoworkFormat)
                    .try_init();
            }
            tracing::info!(
                "log dir: {}",
                log_dir().map_or_else(|| "<disabled>".to_string(), |p| p.display().to_string())
            );
        });
    }

    fn install_file_writer() {
        let Some(dir) = log_dir() else {
            return;
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            #[allow(clippy::print_stderr)]
            {
                eprintln!(
                    "[systemprompt-bridge] cannot create log dir {}: {e}",
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
                #[allow(clippy::print_stderr)]
                {
                    eprintln!("[systemprompt-bridge] rolling appender init failed: {e}");
                }
                return;
            },
        };
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let _ = GUARD.set(guard);
        let _ = FILE_WRITER.set(writer);
    }

    /// Returns the directory holding rotating log files and crash dumps.
    pub fn log_dir() -> Option<PathBuf> {
        platform_log_dir()
    }

    /// Returns today's rotated log file path. Used by support tooling that
    /// wants a single file.
    pub fn log_file_path() -> Option<PathBuf> {
        let dir = log_dir()?;
        let day = chrono::Utc::now().format("%Y-%m-%d");
        Some(dir.join(format!("bridge.log.{day}")))
    }

    #[cfg(target_os = "windows")]
    fn platform_log_dir() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("Claude").join("systemprompt-bridge"))
    }

    #[cfg(target_os = "macos")]
    fn platform_log_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join("Library").join("Logs").join("systemprompt-bridge"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn platform_log_dir() -> Option<PathBuf> {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("state")))
            .map(|base| base.join("systemprompt-bridge"))
    }

    /// Installs a panic hook that writes a crash dump alongside rotating logs
    /// and emits a tracing error event. Must be called before [`init`] so
    /// panics during subscriber setup are still captured.
    pub fn install_panic_hook() {
        std::panic::set_hook(Box::new(|info| {
            let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
            let location = info.location().map_or_else(
                || "<unknown>".to_string(),
                |l| format!("{}:{}", l.file(), l.line()),
            );
            let payload = info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .map(str::to_string)
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_string());
            let backtrace = backtrace::Backtrace::new();
            let dump =
                format!("panic at {location}\npayload: {payload}\n\nbacktrace:\n{backtrace:?}\n");
            if let Some(dir) = log_dir() {
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join(format!("bridge-crash-{ts}.log"));
                let _ = std::fs::write(&path, &dump);
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
            #[allow(clippy::print_stderr)]
            {
                eprintln!("{dump}");
            }
        }));
    }

    struct TeeWriter;

    impl<'a> MakeWriter<'a> for TeeWriter {
        type Writer = TeeWriterImpl;
        fn make_writer(&'a self) -> Self::Writer {
            TeeWriterImpl {
                file: FILE_WRITER.get().cloned(),
            }
        }
    }

    struct TeeWriterImpl {
        file: Option<NonBlocking>,
    }

    impl Write for TeeWriterImpl {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let _ = io::stderr().write_all(buf);
            if let Some(file) = self.file.as_mut() {
                let _ = file.write_all(buf);
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            let _ = io::stderr().flush();
            if let Some(file) = self.file.as_mut() {
                let _ = file.flush();
            }
            Ok(())
        }
    }

    struct CoworkFormat;

    struct MessageVisitor<'a>(&'a mut String);

    impl Visit for MessageVisitor<'_> {
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            if field.name() == "message" {
                self.0.push_str(value);
            }
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
            if field.name() == "message" {
                use std::fmt::Write as _;
                let _ = write!(self.0, "{value:?}");
            }
        }
    }

    impl<S, N> FormatEvent<S, N> for CoworkFormat
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
        N: for<'a> FormatFields<'a> + 'static,
    {
        fn format_event(
            &self,
            _ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
            mut writer: Writer<'_>,
            event: &Event<'_>,
        ) -> fmt::Result {
            let mut message = String::new();
            let mut visitor = MessageVisitor(&mut message);
            event.record(&mut visitor);
            let unquoted = strip_debug_quotes(&message);
            writeln!(writer, "[systemprompt-bridge] {unquoted}")
        }
    }

    fn strip_debug_quotes(s: &str) -> &str {
        if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
            &s[1..s.len() - 1]
        } else {
            s
        }
    }
}
