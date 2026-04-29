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
        tracing::warn!(target: "systemprompt_cowork", "{msg}");
    }
}

pub mod tracing_init {
    use std::fmt;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Write};
    use std::path::PathBuf;
    use std::sync::{Mutex, Once, OnceLock};
    use tracing::{Event, Subscriber};
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::field::Visit;
    use tracing_subscriber::fmt::format::Writer;
    use tracing_subscriber::fmt::{FormatEvent, FormatFields, MakeWriter};
    use tracing_subscriber::registry::LookupSpan;

    static INIT: Once = Once::new();
    static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
    static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

    pub fn init() {
        INIT.call_once(|| {
            let filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
            let _ = tracing_subscriber::fmt()
                .with_writer(TeeWriter)
                .with_env_filter(filter)
                .event_format(CoworkFormat)
                .try_init();
            tracing::info!(
                "log file: {}",
                log_file_path()
                    .map_or_else(|| "<disabled>".to_string(), |p| p.display().to_string())
            );
        });
    }

    pub fn log_file_path() -> Option<PathBuf> {
        LOG_PATH.get_or_init(default_log_path).clone()
    }

    fn default_log_path() -> Option<PathBuf> {
        let dir = log_dir()?;
        if let Err(e) = std::fs::create_dir_all(&dir) {
            #[allow(clippy::print_stderr)]
            {
                eprintln!(
                    "[systemprompt-cowork] cannot create log dir {}: {e}",
                    dir.display()
                );
            }
            return None;
        }
        Some(dir.join("cowork.log"))
    }

    #[cfg(target_os = "windows")]
    fn log_dir() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("Claude").join("systemprompt-cowork"))
    }

    #[cfg(target_os = "macos")]
    fn log_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join("Library").join("Logs").join("systemprompt-cowork"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn log_dir() -> Option<PathBuf> {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("state")))
            .map(|base| base.join("systemprompt-cowork"))
    }

    fn log_file() -> Option<&'static Mutex<File>> {
        if LOG_FILE.get().is_some() {
            return LOG_FILE.get();
        }
        let path = log_file_path()?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()?;
        let _ = LOG_FILE.set(Mutex::new(file));
        LOG_FILE.get()
    }

    struct TeeWriter;

    impl<'a> MakeWriter<'a> for TeeWriter {
        type Writer = TeeWriterImpl;
        fn make_writer(&'a self) -> Self::Writer {
            TeeWriterImpl
        }
    }

    struct TeeWriterImpl;

    impl Write for TeeWriterImpl {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let _ = io::stderr().write_all(buf);
            if let Some(file) = log_file() {
                if let Ok(mut guard) = file.lock() {
                    let _ = guard.write_all(buf);
                }
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            let _ = io::stderr().flush();
            if let Some(file) = log_file() {
                if let Ok(mut guard) = file.lock() {
                    let _ = guard.flush();
                }
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
            writeln!(writer, "[systemprompt-cowork] {unquoted}")
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
