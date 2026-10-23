//! Deferred database-logging tracing layer.
//!
//! [`ProxyDatabaseLayer`] is installed in the subscriber stack before a
//! database pool exists, buffering span attribution into span extensions. Once
//! [`ProxyDatabaseLayer::attach`] supplies a pool it delegates to the real
//! `DatabaseLayer` and hands the caller the writer's [`LogWriterHandle`] (only
//! on the first attach); until then span fields are recorded so attribution is
//! not lost across the boot window. Clones share one state, so the copy inside
//! the installed subscriber and the copy a [`crate::LogSink`] holds see the
//! same sink, instance label, drop count and parked writer. The free functions
//! build the [`LogEntry`] actor triple by walking the span tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use chrono::Utc;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::visitor::{FieldVisitor, SpanContext, SpanFields, SpanVisitor, extract_span_context};
use super::{DatabaseLayer, LayerShared, LogWriterHandle, LogWriterShutdownError};
use crate::models::{LogEntry, LogLevel};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    ClientId, ContextId, InstanceId, LogId, SessionId, TaskId, TraceId, UserId,
};

#[derive(Default)]
struct ProxyState {
    db: OnceLock<DatabaseLayer>,
    shared: LayerShared,
    writer: Mutex<Option<LogWriterHandle>>,
}

#[derive(Clone, Default)]
pub struct ProxyDatabaseLayer {
    inner: Arc<ProxyState>,
}

impl std::fmt::Debug for ProxyDatabaseLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyDatabaseLayer")
            .field("attached", &self.inner.db.get().is_some())
            .finish()
    }
}

impl ProxyDatabaseLayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(&self, db_pool: &DbPool) -> Option<LogWriterHandle> {
        let mut writer = None;
        self.inner.db.get_or_init(|| {
            let (layer, handle) = DatabaseLayer::with_shared(db_pool, self.inner.shared.clone());
            writer = Some(handle);
            layer
        });
        writer
    }

    pub(crate) fn attach_owned(&self, db_pool: &DbPool, instance_id: Option<InstanceId>) {
        if let Some(instance_id) = instance_id {
            self.inner.shared.instance.set(instance_id);
        }
        if let Some(writer) = self.attach(db_pool) {
            *self
                .inner
                .writer
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(writer);
        }
    }

    pub(crate) fn shared(&self) -> &LayerShared {
        &self.inner.shared
    }

    pub(crate) fn enqueue(&self, entry: LogEntry) {
        match self.inner.db.get() {
            Some(db) => db.enqueue(entry),
            None => self.inner.shared.record_drop(),
        }
    }

    pub(crate) async fn shutdown(&self) -> Result<(), LogWriterShutdownError> {
        let writer = self
            .inner
            .writer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        match writer {
            Some(writer) => writer.shutdown().await,
            None => Ok(()),
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
        if let Some(db) = self.inner.db.get() {
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
        if let Some(db) = self.inner.db.get() {
            db.on_record(id, values, ctx);
        } else {
            update_span_fields(id, values, &ctx);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if let Some(db) = self.inner.db.get() {
            db.on_event(event, ctx);
        }
    }
}

pub(super) fn record_span_fields<S>(
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

pub(super) fn update_span_fields<S>(
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

pub(super) fn build_log_entry<S>(event: &Event<'_>, ctx: &Context<'_, S>) -> Option<LogEntry>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let level = *event.metadata().level();
    let module = event.metadata().target().to_owned();

    let mut visitor = FieldVisitor::default();
    event.record(&mut visitor);

    let span_context = ctx
        .current_span()
        .id()
        .and_then(|id| ctx.span(id))
        .map(extract_span_context)?;

    let log_level = match level {
        tracing::Level::ERROR => LogLevel::Error,
        tracing::Level::WARN => LogLevel::Warn,
        tracing::Level::INFO => LogLevel::Info,
        tracing::Level::DEBUG => LogLevel::Debug,
        tracing::Level::TRACE => LogLevel::Trace,
    };

    let user_id = UserId::new(span_context.user.as_ref()?.clone());
    let session_id = SessionId::new(span_context.session.as_ref()?.clone());
    let trace_id = TraceId::new(span_context.trace.as_ref()?.clone());

    Some(LogEntry {
        id: LogId::generate(),
        timestamp: Utc::now(),
        level: log_level,
        module,
        message: visitor.message,
        metadata: visitor.fields,
        user_id,
        session_id,
        task_id: span_context.task.as_ref().map(|s| TaskId::new(s.clone())),
        trace_id,
        context_id: span_context.context.as_ref().and_then(|s| {
            ContextId::try_new(s.clone())
                .map_err(|e| {
                    tracing::warn!(error = %e, raw = %s, "Skipping non-UUID context_id from span context");
                    e
                })
                .ok()
        }),
        client_id: span_context
            .client
            .as_ref()
            .map(|s| ClientId::new(s.clone())),
        instance_id: None,
    })
}
