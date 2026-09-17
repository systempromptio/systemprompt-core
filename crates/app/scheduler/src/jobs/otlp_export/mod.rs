//! The `otlp_export` job: ships this instance's audit trail to an OTLP
//! collector.
//!
//! Each tick, for every signal the profile's `observability.otlp` block
//! names, the job reads the rows after that signal's watermark
//! ([`state`]), converts them to OTLP ([`spans`], [`logs`]), POSTs the
//! envelope ([`transport`]) and, only once the collector has acknowledged
//! it, advances the watermark. A batch that fails keeps its cursor and is
//! retried at the next tick, so nothing is dropped and nothing is skipped;
//! ids are digests of the row keys, so a re-sent batch overwrites rather
//! than duplicates. `batch_seconds` is the lower bound between two exports
//! of a signal; the cron schedule is the upper bound. Metrics are not
//! exported here: they stay on the Prometheus `/metrics` listener.
//!
//! [`export_now`] is the same run without the `batch_seconds` pacing, for a
//! console "export now" action.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod attrs;
mod ids;
mod logs;
mod spans;
mod state;
mod tail;
mod transport;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt_config::ProfileBootstrap;
use systemprompt_database::DbPool;
use systemprompt_models::profile::{OtlpExportConfig, OtlpSignal};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::{debug, info, warn};

pub use ids::{span_id_bytes, trace_id_bytes, unix_nanos};
pub use logs::{severity_number, to_log_record};
pub use spans::{GOVERNANCE_SPAN, REQUEST_SPAN, TOOL_SPAN, TraceBatch, to_spans};
pub use state::{OtlpExportState, OtlpExportStateRepository, Watermark};
pub use tail::{BATCH_ROWS, GovernanceRow, LedgerRow, LogRow, RequestRow, SETTLE};
pub use transport::{BATCHES_TOTAL, RETRY_DELAYS, is_retryable};

use crate::error::{SchedulerError, SchedulerResult};

pub const JOB_NAME: &str = "otlp_export";

/// What one run did for one signal.
#[derive(Debug, Clone, Serialize)]
pub struct SignalReport {
    pub signal: OtlpSignal,
    pub rows: u64,
    pub paced: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExportReport {
    pub signals: Vec<SignalReport>,
}

impl ExportReport {
    #[must_use]
    pub fn rows(&self) -> u64 {
        self.signals.iter().map(|s| s.rows).sum()
    }

    #[must_use]
    pub fn failed(&self) -> u64 {
        self.signals.iter().filter(|s| s.error.is_some()).count() as u64
    }
}

#[must_use]
pub fn pacing_elapsed(
    last_attempt_at: Option<chrono::DateTime<Utc>>,
    now: chrono::DateTime<Utc>,
    batch_seconds: u64,
) -> bool {
    let pace = i64::try_from(batch_seconds)
        .ok()
        .and_then(Duration::try_seconds)
        .unwrap_or(Duration::MAX);
    last_attempt_at.is_none_or(|last| now - last >= pace)
}

#[derive(Debug, Clone, Copy)]
pub struct OtlpExportJob;

#[async_trait]
impl Job for OtlpExportJob {
    fn name(&self) -> &'static str {
        JOB_NAME
    }

    fn description(&self) -> &'static str {
        "Exports AI request spans and logs to the OTLP collector named in observability.otlp"
    }

    fn schedule(&self) -> &'static str {
        "*/15 * * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start = std::time::Instant::now();
        let Some(config) = ProfileBootstrap::get()
            .ok()
            .and_then(|profile| profile.observability.otlp())
        else {
            debug!("observability.otlp is not configured; nothing to export");
            return Ok(JobResult::success().with_duration(start.elapsed().as_millis() as u64));
        };
        let db_pool = ctx
            .db_pool::<DbPool>()
            .ok_or_else(|| SchedulerError::missing_context("DbPool"))?;
        let pool = db_pool.write_pool_arc().map_err(SchedulerError::from)?;
        let instance_id = ctx
            .app_context::<AppContext>()
            .map(|app| app.config().instance_id.clone());

        let report = run(&pool, config, instance_id.as_deref(), true).await?;
        Ok(JobResult::success()
            .with_stats(report.rows(), report.failed())
            .with_duration(start.elapsed().as_millis() as u64))
    }
}

systemprompt_provider_contracts::submit_job!(&OtlpExportJob);

pub async fn export_now(
    pool: &Arc<PgPool>,
    config: &OtlpExportConfig,
    instance_id: Option<&str>,
) -> SchedulerResult<ExportReport> {
    run(pool, config, instance_id, false).await
}

async fn run(
    pool: &Arc<PgPool>,
    config: &OtlpExportConfig,
    instance_id: Option<&str>,
    paced: bool,
) -> SchedulerResult<ExportReport> {
    let repository = OtlpExportStateRepository::new(pool.as_ref().clone());
    let mut report = ExportReport::default();
    for signal in OtlpSignal::ALL {
        if !config.exports(signal) {
            continue;
        }
        let state = repository.get_or_start(signal).await?;
        if paced && !pacing_elapsed(state.last_attempt_at, Utc::now(), config.batch_seconds) {
            report.signals.push(SignalReport {
                signal,
                rows: 0,
                paced: true,
                error: None,
            });
            continue;
        }
        repository.mark_attempt(signal).await?;
        let outcome = export_signal(pool, config, &repository, &state, instance_id).await;
        report.signals.push(match outcome {
            Ok(rows) => SignalReport {
                signal,
                rows,
                paced: false,
                error: None,
            },
            Err(error) => {
                let message = error.to_string();
                warn!(signal = %signal, error = %message, "OTLP export batch failed");
                repository.record_failure(signal, &message).await?;
                SignalReport {
                    signal,
                    rows: 0,
                    paced: false,
                    error: Some(message),
                }
            },
        });
    }
    Ok(report)
}

async fn export_signal(
    pool: &PgPool,
    config: &OtlpExportConfig,
    repository: &OtlpExportStateRepository,
    state: &OtlpExportState,
    instance_id: Option<&str>,
) -> SchedulerResult<u64> {
    let after = state.watermark();
    let signal = OtlpSignal::parse(&state.signal)
        .ok_or_else(|| SchedulerError::Internal(format!("unknown signal {}", state.signal)))?;
    let (rows, next) = match signal {
        OtlpSignal::Traces => {
            let batch = load_trace_batch(pool, &after).await?;
            let next = batch
                .requests
                .last()
                .map(|r| Watermark::new(r.completed_at, r.id.clone()));
            let rows = batch.requests.len() as u64;
            if next.is_some() {
                let envelope = spans::to_export_request(&batch, instance_id);
                post(config, signal, &envelope).await?;
            }
            (rows, next)
        },
        OtlpSignal::Logs => {
            let batch = tail::list_logs_after(pool, &after, BATCH_ROWS).await?;
            let next = batch
                .last()
                .map(|r| Watermark::new(r.timestamp, r.id.clone()));
            if next.is_some() {
                let envelope = logs::to_export_request(&batch, instance_id);
                post(config, signal, &envelope).await?;
            }
            (batch.len() as u64, next)
        },
    };
    match next {
        Some(next) => {
            repository.advance(signal, &next, rows as i64).await?;
            info!(signal = %signal, rows, "OTLP batch exported");
        },
        None => repository.mark_caught_up(signal, SETTLE).await?,
    }
    Ok(rows)
}

async fn load_trace_batch(pool: &PgPool, after: &Watermark) -> SchedulerResult<TraceBatch> {
    let requests = tail::list_requests_after(pool, after, BATCH_ROWS).await?;
    if requests.is_empty() {
        return Ok(TraceBatch::default());
    }
    let ids: Vec<String> = requests.iter().map(|r| r.id.clone()).collect();
    let mut batch = TraceBatch {
        requests,
        ..TraceBatch::default()
    };
    batch.ledger = tail::list_ledger_for_requests(pool, &ids).await?;
    let trace_ids = batch.trace_ids();
    if !trace_ids.is_empty() {
        batch.governance = tail::list_governance_for_traces(pool, &trace_ids).await?;
    }
    Ok(batch)
}

async fn post<M: prost::Message>(
    config: &OtlpExportConfig,
    signal: OtlpSignal,
    envelope: &M,
) -> SchedulerResult<()> {
    transport::post_signal(config, signal, envelope)
        .await
        .map_err(|e| SchedulerError::Internal(e.to_string()))
}
