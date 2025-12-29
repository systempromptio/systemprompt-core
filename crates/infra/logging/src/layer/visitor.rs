use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::registry::LookupSpan;

mod field_names {
    pub const MESSAGE: &str = "message";
    pub const USER_ID: &str = "user_id";
    pub const SESSION_ID: &str = "session_id";
    pub const TASK_ID: &str = "task_id";
    pub const TRACE_ID: &str = "trace_id";
    pub const CONTEXT_ID: &str = "context_id";
    pub const CLIENT_ID: &str = "client_id";
}

#[derive(Default)]
pub struct FieldVisitor {
    pub message: String,
    pub fields: Option<serde_json::Value>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == field_names::MESSAGE {
            self.message = format!("{value:?}");
        } else {
            let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
            if let Some(obj) = fields.as_object_mut() {
                obj.insert(
                    field.name().to_string(),
                    serde_json::json!(format!("{value:?}")),
                );
            }
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == field_names::MESSAGE {
            self.message = value.to_string();
        } else {
            let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
            if let Some(obj) = fields.as_object_mut() {
                obj.insert(field.name().to_string(), serde_json::json!(value));
            }
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = fields.as_object_mut() {
            obj.insert(field.name().to_string(), serde_json::json!(value));
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = fields.as_object_mut() {
            obj.insert(field.name().to_string(), serde_json::json!(value));
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        let fields = self.fields.get_or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = fields.as_object_mut() {
            obj.insert(field.name().to_string(), serde_json::json!(value));
        }
    }
}

#[derive(Default)]
pub struct SpanContext {
    pub user: Option<String>,
    pub session: Option<String>,
    pub task: Option<String>,
    pub trace: Option<String>,
    pub context: Option<String>,
    pub client: Option<String>,
}

pub struct SpanVisitor<'a> {
    pub context: &'a mut SpanContext,
}

impl Visit for SpanVisitor<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value_str = format!("{value:?}");
        self.record_field(field.name(), value_str);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_field(field.name(), value.to_string());
    }
}

impl SpanVisitor<'_> {
    fn record_field(&mut self, name: &str, value: String) {
        match name {
            field_names::USER_ID => self.context.user = Some(value),
            field_names::SESSION_ID => self.context.session = Some(value),
            field_names::TASK_ID => {
                if !value.is_empty() {
                    self.context.task = Some(value);
                }
            },
            field_names::TRACE_ID => self.context.trace = Some(value),
            field_names::CONTEXT_ID => {
                if !value.is_empty() {
                    self.context.context = Some(value);
                }
            },
            field_names::CLIENT_ID => {
                if !value.is_empty() {
                    self.context.client = Some(value);
                }
            },
            _ => {},
        }
    }
}

#[derive(Default, Clone)]
pub struct SpanFields {
    pub user: Option<String>,
    pub session: Option<String>,
    pub task: Option<String>,
    pub trace: Option<String>,
    pub context: Option<String>,
    pub client: Option<String>,
}

pub fn extract_span_context<S>(span: tracing_subscriber::registry::SpanRef<'_, S>) -> SpanContext
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
