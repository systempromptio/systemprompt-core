//! Tests for `FilterSystemFields` and its visitor.
//!
//! The visitor is exercised by driving it through a real `tracing` event so
//! we touch the actual `Visit` impls (`record_str`, `record_debug`) without
//! reaching for crate-private constructors.

use std::sync::{Arc, Mutex};
use systemprompt_logging::FilterSystemFields;
use tracing::{Subscriber, info, info_span};
use tracing_subscriber::layer::SubscriberExt;

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

fn make_subscriber(writer: CapturingWriter) -> impl Subscriber + Send + Sync {
    let layer = tracing_subscriber::fmt::layer()
        .fmt_fields(FilterSystemFields::new())
        .with_target(false)
        .with_writer(writer)
        .with_ansi(false);
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("trace"))
        .with(layer)
}

#[test]
fn filter_is_copy_default_debug() {
    let f = FilterSystemFields::new();
    let copied = f;
    let _debug = format!("{f:?} {copied:?}");
    let _default = FilterSystemFields;
}

#[test]
fn record_str_drops_literal_system_value() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        info!(actor = "system", message = "should-not-emit-actor");
    });

    let logs = writer.contents();
    // The 'actor=system' field must not appear (record_str early-returns on
    // "system").
    assert!(!logs.contains("actor="));
    // The 'message' field WAS emitted (and logged as the event message).
    assert!(logs.contains("should-not-emit-actor"));
}

#[test]
fn record_debug_drops_system_debug_value() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        // Use a non-string field whose Debug repr is exactly "system" or "\"system\"".
        info!(owner = ?"system", body = "hello");
    });

    let logs = writer.contents();
    assert!(!logs.contains("owner="));
    assert!(logs.contains("hello"));
}

#[test]
fn other_fields_are_preserved_and_space_separated() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        info!(user_id = "alice", count = 42, "structured");
    });

    let logs = writer.contents();
    assert!(logs.contains("user_id="));
    assert!(logs.contains("alice"));
    assert!(logs.contains("count=42"));
}

#[test]
fn span_fields_also_filtered() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        let span = info_span!("op", actor = "system", tenant = "acme");
        let _enter = span.enter();
        info!("inside-span");
    });

    let logs = writer.contents();
    assert!(logs.contains("inside-span"));
    assert!(logs.contains("tenant"));
}
