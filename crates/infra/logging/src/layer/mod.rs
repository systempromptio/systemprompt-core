#![allow(clippy::print_stderr)] // Fallback when logging fails

mod visitor;

use std::time::Duration;

use chrono::Utc;
use tokio::sync::mpsc;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use crate::models::{LogEntry, LogLevel};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{ClientId, ContextId, LogId, SessionId, TaskId, TraceId, UserId};
use visitor::{extract_span_context, FieldVisitor, SpanContext, SpanFields, SpanVisitor};

const BUFFER_FLUSH_SIZE: usize = 100;
const BUFFER_FLUSH_INTERVAL_SECS: u64 = 10;

pub struct DatabaseLayer {
    sender: mpsc::UnboundedSender<LogEntry>,
}

impl std::fmt::Debug for DatabaseLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseLayer").finish_non_exhaustive()
    }
}

impl DatabaseLayer {
    pub fn new(db_pool: DbPool) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        tokio::spawn(Self::batch_writer(db_pool, receiver));

        Self { sender }
    }

    async fn batch_writer(db_pool: DbPool, mut receiver: mpsc::UnboundedReceiver<LogEntry>) {
        let mut buffer = Vec::with_capacity(BUFFER_FLUSH_SIZE);
        let mut interval = tokio::time::interval(Duration::from_secs(BUFFER_FLUSH_INTERVAL_SECS));

        loop {
            tokio::select! {
                Some(entry) = receiver.recv() => {
                    buffer.push(entry);
                    if buffer.len() >= BUFFER_FLUSH_SIZE {
                        Self::flush(&db_pool, &mut buffer).await;
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        Self::flush(&db_pool, &mut buffer).await;
                    }
                }
            }
        }
    }

    async fn flush(db_pool: &DbPool, buffer: &mut Vec<LogEntry>) {
        if let Err(e) = Self::batch_insert(db_pool, buffer).await {
            eprintln!("Failed to flush logs: {e}");
        }
        buffer.clear();
    }

    async fn batch_insert(db_pool: &DbPool, entries: &[LogEntry]) -> anyhow::Result<()> {
        let pool = db_pool.pool_arc()?;
        for entry in entries {
            let metadata_json: Option<String> = entry
                .metadata
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;

            let entry_id = entry.id.as_str();
            let level_str = entry.level.to_string();
            let user_id = entry.user_id.as_str();
            let session_id = entry.session_id.as_str();
            let task_id = entry.task_id.as_ref().map(TaskId::as_str);
            let trace_id = entry.trace_id.as_str();
            let context_id = entry.context_id.as_ref().map(ContextId::as_str);
            let client_id = entry.client_id.as_ref().map(ClientId::as_str);

            sqlx::query!(
                r"
                INSERT INTO logs (id, level, module, message, metadata, user_id, session_id, task_id, trace_id, context_id, client_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ",
                entry_id,
                level_str,
                entry.module,
                entry.message,
                metadata_json,
                user_id,
                session_id,
                task_id,
                trace_id,
                context_id,
                client_id
            )
            .execute(pool.as_ref())
            .await?;
        }

        Ok(())
    }
}

impl<S> Layer<S> for DatabaseLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
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

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: Context<'_, S>,
    ) {
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

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
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

        let entry = LogEntry {
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
        };

        let _ = self.sender.send(entry);
    }
}
