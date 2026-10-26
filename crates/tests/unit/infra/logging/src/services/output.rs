//! Unit tests for output mode functions

use std::sync::{Arc, Mutex};
use systemprompt_logging::{FilterSystemFields, OutputState};
use tracing::{Level, Subscriber, info, warn};
use tracing_subscriber::filter::FilterFn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Layer};

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

fn make_dynamic_subscriber(
    writer: CapturingWriter,
    output: OutputState,
) -> impl Subscriber + Send + Sync {
    let base = EnvFilter::new("info");
    let gate = FilterFn::new(move |meta| !output.is_startup_mode() || *meta.level() <= Level::WARN);
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
    let output = OutputState::new();
    let _result = output.is_startup_mode();
    output.set_startup_mode(true);
    assert!(output.is_startup_mode());
}

#[test]
fn test_set_startup_mode_true() {
    let output = OutputState::new();
    output.set_startup_mode(true);
    assert!(output.is_startup_mode());
}

#[test]
fn test_set_startup_mode_false() {
    let output = OutputState::new();
    output.set_startup_mode(false);
    assert!(!output.is_startup_mode());
}

#[test]
fn test_startup_mode_toggle() {
    let output = OutputState::new();
    output.set_startup_mode(true);
    assert!(output.is_startup_mode());

    output.set_startup_mode(false);
    assert!(!output.is_startup_mode());

    output.set_startup_mode(true);
    assert!(output.is_startup_mode());
}

#[test]
fn dynamic_filter_follows_startup_mode_after_init() {
    let output = OutputState::new();
    let writer = CapturingWriter::default();
    let subscriber = make_dynamic_subscriber(writer.clone(), output.clone());

    tracing::subscriber::with_default(subscriber, || {
        output.set_startup_mode(true);
        info!("startup-info-suppressed");
        warn!("startup-warn-shown");
        output.set_startup_mode(false);
        info!("ready-info-shown");
    });

    let logs = writer.contents();
    assert!(!logs.contains("startup-info-suppressed"));
    assert!(logs.contains("startup-warn-shown"));
    assert!(logs.contains("ready-info-shown"));
}
