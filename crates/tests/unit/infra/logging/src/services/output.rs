//! Unit tests for output mode functions

use std::sync::{Arc, Mutex};
use systemprompt_logging::{FilterSystemFields, is_startup_mode, set_startup_mode};
use tracing::{Level, Subscriber, info, warn};
use tracing_subscriber::filter::FilterFn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Layer};

static STARTUP_MODE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Default)]
struct CapturingWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl CapturingWriter {
    fn contents(&self) -> String {
        let buf = self.buffer.lock().expect("buffer poisoned");
        String::from_utf8(buf.clone()).expect("utf8 in captured logs")
    }
}

impl std::io::Write for CapturingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer
            .lock()
            .expect("buffer poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturingWriter {
    type Writer = CapturingWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn make_dynamic_subscriber(writer: CapturingWriter) -> impl Subscriber + Send + Sync {
    let base = EnvFilter::new("info");
    let gate = FilterFn::new(|meta| !is_startup_mode() || *meta.level() <= Level::WARN);
    let layer = tracing_subscriber::fmt::layer()
        .fmt_fields(FilterSystemFields::new())
        .with_target(false)
        .with_writer(writer)
        .with_ansi(false)
        .with_filter(base)
        .with_filter(gate);
    tracing_subscriber::registry().with(layer)
}

#[test]
fn test_startup_mode_returns_bool() {
    let _guard = STARTUP_MODE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let _result = is_startup_mode();
    set_startup_mode(true);
    assert!(is_startup_mode());
}

#[test]
fn test_set_startup_mode_true() {
    let _guard = STARTUP_MODE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    set_startup_mode(true);
    assert!(is_startup_mode());
}

#[test]
fn test_set_startup_mode_false() {
    let _guard = STARTUP_MODE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    set_startup_mode(false);
    assert!(!is_startup_mode());
}

#[test]
fn test_startup_mode_toggle() {
    let _guard = STARTUP_MODE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    set_startup_mode(true);
    assert!(is_startup_mode());

    set_startup_mode(false);
    assert!(!is_startup_mode());

    set_startup_mode(true);
    assert!(is_startup_mode());
}

#[test]
fn dynamic_filter_follows_startup_mode_after_init() {
    let _guard = STARTUP_MODE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let writer = CapturingWriter::default();
    let subscriber = make_dynamic_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        set_startup_mode(true);
        info!("startup-info-suppressed");
        warn!("startup-warn-shown");
        set_startup_mode(false);
        info!("ready-info-shown");
    });

    let logs = writer.contents();
    assert!(!logs.contains("startup-info-suppressed"));
    assert!(logs.contains("startup-warn-shown"));
    assert!(logs.contains("ready-info-shown"));
}
