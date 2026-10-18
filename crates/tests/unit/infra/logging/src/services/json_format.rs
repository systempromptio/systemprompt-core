//! The JSON console formatter: one object per event, redacted like text.

use std::sync::{Arc, Mutex};

use systemprompt_logging::{
    FilterSystemFields, JsonEventFormat, console_format, set_console_format,
};
use systemprompt_manifest::profile::LogFormat;
use tracing::{Subscriber, info, info_span};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Clone, Default)]
struct CapturingWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl CapturingWriter {
    fn lines(&self) -> Vec<serde_json::Value> {
        let buf = self.buffer.lock().expect("buffer poisoned");
        String::from_utf8(buf.clone())
            .expect("utf8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("each line is one JSON object"))
            .collect()
    }

    fn raw(&self) -> String {
        String::from_utf8(self.buffer.lock().expect("buffer poisoned").clone()).expect("utf8")
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

fn json_subscriber(writer: CapturingWriter) -> impl Subscriber + Send + Sync {
    let layer = tracing_subscriber::fmt::layer()
        .fmt_fields(FilterSystemFields::new())
        .event_format(JsonEventFormat)
        .with_writer(writer)
        .with_ansi(false);
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("trace"))
        .with(layer)
}

#[test]
fn an_event_is_one_json_object_with_level_target_and_typed_fields() {
    let writer = CapturingWriter::default();
    tracing::subscriber::with_default(json_subscriber(writer.clone()), || {
        info!(
            user_id = "u-1",
            attempts = 3_u64,
            ok = true,
            "request served"
        );
    });

    let lines = writer.lines();
    assert_eq!(lines.len(), 1);
    let event = &lines[0];
    assert_eq!(event["level"], "INFO");
    assert!(event["target"].as_str().is_some_and(|t| !t.is_empty()));
    assert!(event["timestamp"].as_str().is_some());
    assert_eq!(event["fields"]["message"], "request served");
    assert_eq!(event["fields"]["user_id"], "u-1");
    assert_eq!(event["fields"]["attempts"], 3);
    assert_eq!(event["fields"]["ok"], true);
}

#[test]
fn a_secret_bearing_field_is_redacted_in_json_output() {
    let writer = CapturingWriter::default();
    tracing::subscriber::with_default(json_subscriber(writer.clone()), || {
        info!(
            api_key = "sk-live-do-not-print",
            authorization = "Bearer abc.def",
            password = 42_u64,
            "outbound call"
        );
    });

    let raw = writer.raw();
    assert!(!raw.contains("sk-live-do-not-print"), "{raw}");
    assert!(!raw.contains("abc.def"), "{raw}");
    let event = &writer.lines()[0];
    assert_eq!(event["fields"]["api_key"], "[REDACTED]");
    assert_eq!(event["fields"]["authorization"], "[REDACTED]");
    assert_eq!(event["fields"]["password"], "[REDACTED]");
}

#[test]
fn span_fields_are_rendered_through_the_redacting_field_formatter() {
    let writer = CapturingWriter::default();
    tracing::subscriber::with_default(json_subscriber(writer.clone()), || {
        let span = info_span!("gateway", token = "tok-secret", route = "messages");
        let _entered = span.enter();
        info!("inside");
    });

    let raw = writer.raw();
    assert!(!raw.contains("tok-secret"), "{raw}");
    let event = &writer.lines()[0];
    assert_eq!(event["spans"][0]["name"], "gateway");
    let fields = event["spans"][0]["fields"].as_str().expect("span fields");
    assert!(fields.contains("route="), "{fields}");
}

#[test]
fn the_console_format_switch_round_trips() {
    set_console_format(LogFormat::Json);
    assert_eq!(console_format(), LogFormat::Json);
    set_console_format(LogFormat::Text);
    assert_eq!(console_format(), LogFormat::Text);
}
