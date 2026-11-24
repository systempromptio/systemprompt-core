//! The bounded feed and the owned task that ships it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::{OtlpExportConfig, OtlpSignal};
use systemprompt_traits::BackgroundTasks;
use tokio::sync::mpsc;

use super::attrs::{resource, scope};
use super::records::{ClosedSpan, to_log_record};
use super::transport;
use crate::models::LogEntry;

pub const DROPPED_TOTAL: &str = "otlp_export_dropped_total";
pub const QUEUE_CAPACITY: usize = 8192;
pub const BATCH_ITEMS: usize = 512;

enum Item {
    Log(Box<LogRecord>),
    Span(Box<Span>),
}

/// The pipeline's handle onto the exporter queue; clones share it.
#[derive(Clone)]
pub struct OtlpFeed {
    sender: mpsc::Sender<Item>,
    dropped: Arc<AtomicU64>,
    traces: bool,
    logs: bool,
}

impl std::fmt::Debug for OtlpFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OtlpFeed")
            .field("dropped", &self.dropped())
            .finish_non_exhaustive()
    }
}

impl OtlpFeed {
    #[must_use]
    pub fn spawn(
        config: &OtlpExportConfig,
        instance_id: Option<InstanceId>,
        tasks: &BackgroundTasks,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let shipper = Shipper {
            config: config.clone(),
            instance_id,
            client: transport::build_client(),
            dropped: Arc::new(AtomicU64::new(0)),
            logs: Vec::new(),
            spans: Vec::new(),
        };
        let feed = Self {
            sender,
            dropped: Arc::clone(&shipper.dropped),
            traces: config.exports(OtlpSignal::Traces),
            logs: config.exports(OtlpSignal::Logs),
        };
        tasks.spawn_cancellable("otlp_export", move |cancel| async move {
            shipper.run(receiver, cancel.cancelled_owned()).await;
        });
        feed
    }

    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn log(&self, entry: &LogEntry) {
        if self.logs {
            self.offer(OtlpSignal::Logs, Item::Log(Box::new(to_log_record(entry))));
        }
    }

    pub(crate) fn span(&self, span: ClosedSpan) {
        if self.traces {
            self.offer(OtlpSignal::Traces, Item::Span(Box::new(span.into_span())));
        }
    }

    pub fn audit_span(&self, span: Span) {
        if self.traces {
            self.offer(OtlpSignal::Traces, Item::Span(Box::new(span)));
        }
    }

    fn offer(&self, signal: OtlpSignal, item: Item) {
        if self.sender.try_send(item).is_err() {
            count_dropped(&self.dropped, signal, 1);
        }
    }
}

fn count_dropped(dropped: &AtomicU64, signal: OtlpSignal, n: u64) {
    dropped.fetch_add(n, Ordering::Relaxed);
    metrics::counter!(DROPPED_TOTAL, "signal" => signal.label()).increment(n);
}

struct Shipper {
    config: OtlpExportConfig,
    instance_id: Option<InstanceId>,
    client: reqwest::Client,
    dropped: Arc<AtomicU64>,
    logs: Vec<LogRecord>,
    spans: Vec<Span>,
}

impl Shipper {
    async fn run(
        mut self,
        mut receiver: mpsc::Receiver<Item>,
        cancelled: impl Future<Output = ()>,
    ) {
        tokio::pin!(cancelled);
        let mut tick = tokio::time::interval(Duration::from_secs(self.config.batch_seconds.max(1)));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = &mut cancelled => break,
                item = receiver.recv() => {
                    let Some(item) = item else { break };
                    self.push(item);
                    if self.logs.len() + self.spans.len() >= BATCH_ITEMS {
                        self.flush().await;
                    }
                },
                _ = tick.tick() => self.flush().await,
            }
        }
        receiver.close();
        while let Ok(item) = receiver.try_recv() {
            self.push(item);
        }
        self.flush().await;
    }

    fn push(&mut self, item: Item) {
        match item {
            Item::Log(record) => self.logs.push(*record),
            Item::Span(span) => self.spans.push(*span),
        }
    }

    async fn flush(&mut self) {
        let spans = std::mem::take(&mut self.spans);
        if !spans.is_empty() {
            let n = spans.len() as u64;
            let envelope = ExportTraceServiceRequest {
                resource_spans: vec![ResourceSpans {
                    resource: Some(resource(self.instance_id.as_ref())),
                    scope_spans: vec![ScopeSpans {
                        scope: Some(scope()),
                        spans,
                        schema_url: String::new(),
                    }],
                    schema_url: String::new(),
                }],
            };
            self.post(OtlpSignal::Traces, n, &envelope).await;
        }
        let logs = std::mem::take(&mut self.logs);
        if !logs.is_empty() {
            let n = logs.len() as u64;
            let envelope = ExportLogsServiceRequest {
                resource_logs: vec![ResourceLogs {
                    resource: Some(resource(self.instance_id.as_ref())),
                    scope_logs: vec![ScopeLogs {
                        scope: Some(scope()),
                        log_records: logs,
                        schema_url: String::new(),
                    }],
                    schema_url: String::new(),
                }],
            };
            self.post(OtlpSignal::Logs, n, &envelope).await;
        }
    }

    async fn post<M: prost::Message>(&self, signal: OtlpSignal, n: u64, envelope: &M) {
        if let Err(error) =
            transport::post_signal(&self.client, &self.config, signal, envelope).await
        {
            count_dropped(&self.dropped, signal, n);
            tracing::warn!(signal = %signal, items = n, %error, "OTLP batch dropped");
        }
    }
}
