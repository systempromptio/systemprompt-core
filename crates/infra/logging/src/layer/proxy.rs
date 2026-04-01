use std::io::Write;
use std::sync::{Arc, OnceLock};

use chrono::Utc;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::DatabaseLayer;
use super::visitor::{FieldVisitor, SpanContext, SpanFields, SpanVisitor, extract_span_context};
use crate::models::{LogEntry, LogLevel};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ClientId, ContextId, LogId, SessionId, TaskId, TraceId, UserId};

#[derive(Clone)]
pub struct ProxyDatabaseLayer {
    inner: Arc<OnceLock<DatabaseLayer>>,
}

impl std::fmt::Debug for ProxyDatabaseLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyDatabaseLayer")
            .field("attached", &self.inner.get().is_some())
            .finish()
    }
}

impl Default for ProxyDatabaseLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyDatabaseLayer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(OnceLock::new()),
        }
    }

    pub fn attach(&self, db_pool: DbPool) {
        if self.inner.set(DatabaseLayer::new(db_pool)).is_err() {
            let _ = writeln!(
                std::io::stderr(),
                "ProxyDatabaseLayer: database layer already attached, ignoring duplicate"
            );
        }
    }
}

impl<S> Layer<S> for ProxyDatabaseLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        if let Some(db) = self.inner.get() {
            db.on_new_span(attrs, id, ctx);
        } else {
            record_span_fields(attrs, id, &ctx);
        }
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: Context<'_, S>,
    ) {
        if let Some(db) = self.inner.get() {
            db.on_record(id, values, ctx);
        } else {
            update_span_fields(id, values, &ctx);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if let Some(db) = self.inner.get() {
            db.on_event(event, ctx);
        }
    }
}

pub fn record_span_fields<S>(
    attrs: &tracing::span::Attributes<'_>,
    id: &tracing::span::Id,
    ctx: &Context<'_, S>,
) where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let Some(span) = ctx.span(id) else {
        return;
    };
    let mut fields = SpanFields::default();
    let mut context = SpanContext::default();
    let mut visitor = SpanVisitor {
        context: &mut context,
    };
    attrs.record(&mut visitor);

    fields.user = context.user;
    fields.session = context.session;
    fields.task = context.task;
    fields.trace = context.trace;
    fields.context = context.context;
    fields.client = context.client;

    let mut extensions = span.extensions_mut();
    extensions.insert(fields);
}

pub fn update_span_fields<S>(
    id: &tracing::span::Id,
    values: &tracing::span::Record<'_>,
    ctx: &Context<'_, S>,
) where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    if let Some(span) = ctx.span(id) {
        let mut extensions = span.extensions_mut();
        if let Some(fields) = extensions.get_mut::<SpanFields>() {
            let mut context = SpanContext {
                user: fields.user.clone(),
                session: fields.session.clone(),
                task: fields.task.clone(),
                trace: fields.trace.clone(),
                context: fields.context.clone(),
                client: fields.client.clone(),
            };
            let mut visitor = SpanVisitor {
                context: &mut context,
            };
            values.record(&mut visitor);

            fields.user = context.user;
            fields.session = context.session;
            fields.task = context.task;
            fields.trace = context.trace;
            fields.context = context.context;
            fields.client = context.client;
        }
    }
}

pub fn build_log_entry<S>(event: &Event<'_>, ctx: &Context<'_, S>) -> LogEntry
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let level = *event.metadata().level();
    let module = event.metadata().target().to_string();

    let mut visitor = FieldVisitor::default();
    event.record(&mut visitor);

    let span_context = ctx
        .current_span()
        .id()
        .and_then(|id| ctx.span(id))
        .map(extract_span_context);

    let log_level = match level {
        tracing::Level::ERROR => LogLevel::Error,
        tracing::Level::WARN => LogLevel::Warn,
        tracing::Level::INFO => LogLevel::Info,
        tracing::Level::DEBUG => LogLevel::Debug,
        tracing::Level::TRACE => LogLevel::Trace,
    };

    LogEntry {
        id: LogId::generate(),
        timestamp: Utc::now(),
        level: log_level,
        module,
        message: visitor.message,
        metadata: visitor.fields,
        user_id: span_context
            .as_ref()
            .and_then(|c| c.user.as_ref())
            .map_or_else(UserId::system, |s| UserId::new(s.clone())),
        session_id: span_context
            .as_ref()
            .and_then(|c| c.session.as_ref())
            .map_or_else(SessionId::system, |s| SessionId::new(s.clone())),
        task_id: span_context
            .as_ref()
            .and_then(|c| c.task.as_ref())
            .map(|s| TaskId::new(s.clone())),
        trace_id: span_context
            .as_ref()
            .and_then(|c| c.trace.as_ref())
            .map_or_else(TraceId::system, |s| TraceId::new(s.clone())),
        context_id: span_context
            .as_ref()
            .and_then(|c| c.context.as_ref())
            .map(|s| ContextId::new(s.clone())),
        client_id: span_context
            .as_ref()
            .and_then(|c| c.client.as_ref())
            .map(|s| ClientId::new(s.clone())),
    }
}
