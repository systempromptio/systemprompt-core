//! Tracing field visitors for the database log layer.
//!
//! [`FieldVisitor`] collects event fields into a JSON metadata blob, redacting
//! a fixed set of sensitive field names so secrets never reach the log store.
//! [`SpanVisitor`]/[`SpanContext`]/[`SpanFields`] capture the identifier fields
//! attached to spans, and [`extract_span_context`] walks a span's ancestors to
//! resolve the full attribution context for an event.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use tracing::Subscriber;
use tracing::field::{Field, Visit};
use tracing_subscriber::registry::LookupSpan;

use crate::sanitize::is_redacted;

mod field_names {
    pub(super) const MESSAGE: &str = "message";
    pub(super) const USER_ID: &str = "user_id";
    pub(super) const SESSION_ID: &str = "session_id";
    pub(super) const TASK_ID: &str = "task_id";
    pub(super) const TRACE_ID: &str = "trace_id";
    pub(super) const CONTEXT_ID: &str = "context_id";
    pub(super) const CLIENT_ID: &str = "client_id";
}

#[derive(Debug, Default)]
pub(super) struct FieldVisitor {
    pub message: String,
    pub fields: Option<serde_json::Value>,
}

// Why: Strips ANSI CSI escape sequences (`ESC [ … final-byte`) from `input` so
// the stored message is plain text. Scope is intentionally narrow: only CSI
// sequences (the colour/style codes the console fmt layer emits) are removed.
// Other escape forms (OSC, single-character escapes) have just the lone `ESC`
// dropped; their payload is preserved rather than guessed at.
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1B' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == field_names::MESSAGE {
            self.message = strip_ansi(&format!("{value:?}"));
        } else {
            let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
            if let Some(obj) = fields.as_object_mut() {
                let rendered = if is_redacted(field.name()) {
                    serde_json::json!("[REDACTED]")
                } else {
                    serde_json::json!(format!("{value:?}"))
                };
                obj.insert(field.name().to_owned(), rendered);
            }
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == field_names::MESSAGE {
            self.message = strip_ansi(value);
        } else {
            let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
            if let Some(obj) = fields.as_object_mut() {
                let rendered = if is_redacted(field.name()) {
                    serde_json::json!("[REDACTED]")
                } else {
                    serde_json::json!(value)
                };
                obj.insert(field.name().to_owned(), rendered);
            }
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = fields.as_object_mut() {
            let rendered = if is_redacted(field.name()) {
                serde_json::json!("[REDACTED]")
            } else {
                serde_json::json!(value)
            };
            obj.insert(field.name().to_owned(), rendered);
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = fields.as_object_mut() {
            let rendered = if is_redacted(field.name()) {
                serde_json::json!("[REDACTED]")
            } else {
                serde_json::json!(value)
            };
            obj.insert(field.name().to_owned(), rendered);
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = fields.as_object_mut() {
            let rendered = if is_redacted(field.name()) {
                serde_json::json!("[REDACTED]")
            } else {
                serde_json::json!(value)
            };
            obj.insert(field.name().to_owned(), rendered);
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct SpanContext {
    pub user: Option<String>,
    pub session: Option<String>,
    pub task: Option<String>,
    pub trace: Option<String>,
    pub context: Option<String>,
    pub client: Option<String>,
}

#[derive(Debug)]
pub(super) struct SpanVisitor<'a> {
    pub context: &'a mut SpanContext,
}

impl Visit for SpanVisitor<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value_str = format!("{value:?}");
        self.record_field(field.name(), value_str);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_field(field.name(), value.to_owned());
    }
}

impl SpanVisitor<'_> {
    fn record_field(&mut self, name: &str, value: String) {
        match name {
            field_names::USER_ID => self.context.user = Some(value),
            field_names::SESSION_ID => self.context.session = Some(value),
            field_names::TASK_ID if !value.is_empty() => {
                self.context.task = Some(value);
            },
            field_names::TRACE_ID => self.context.trace = Some(value),
            field_names::CONTEXT_ID if !value.is_empty() => {
                self.context.context = Some(value);
            },
            field_names::CLIENT_ID if !value.is_empty() => {
                self.context.client = Some(value);
            },
            _ => {},
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(super) struct SpanFields {
    pub user: Option<String>,
    pub session: Option<String>,
    pub task: Option<String>,
    pub trace: Option<String>,
    pub context: Option<String>,
    pub client: Option<String>,
}

pub(super) fn extract_span_context<S>(
    span: tracing_subscriber::registry::SpanRef<'_, S>,
) -> SpanContext
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let mut context = SpanContext::default();

    let mut current = Some(span);
    while let Some(span_ref) = current {
        {
            let extensions = span_ref.extensions();
            if let Some(fields) = extensions.get::<SpanFields>() {
                if context.user.is_none() {
                    context.user.clone_from(&fields.user);
                }
                if context.session.is_none() {
                    context.session.clone_from(&fields.session);
                }
                if context.task.is_none() {
                    context.task.clone_from(&fields.task);
                }
                if context.trace.is_none() {
                    context.trace.clone_from(&fields.trace);
                }
                if context.context.is_none() {
                    context.context.clone_from(&fields.context);
                }
                if context.client.is_none() {
                    context.client.clone_from(&fields.client);
                }
            }
        }
        current = span_ref.parent();
    }

    context
}
