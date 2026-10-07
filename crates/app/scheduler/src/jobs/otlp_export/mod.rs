//! The `otlp_export` job: ships this instance's audit trail to an OTLP
//! collector.
//!
//! Each tick, for every signal the profile's `observability.otlp` block
//! names, the job reads the rows after that signal's watermark
//! ([`crate::repository::otlp`]), converts them to OTLP (`spans`, `logs`),
//! POSTs the envelope (`transport`) and, only once the collector has
//! acknowledged it, advances the watermark. A batch that fails keeps its cursor
//! and is retried at the next tick, so nothing is dropped. A row whose stored
//! identifier is malformed is the one exception: it is skipped, counted in
//! the [`SignalReport`] and logged, and the rest of its batch still ships;
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
mod report;
mod spans;
mod transport;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_config::ProfileBootstrap;
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{AiRequestId, InstanceId};
use systemprompt_manifest::profile::{OtlpExportConfig, OtlpSignal};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::{debug, info, warn};

pub use ids::{span_id_bytes, trace_id_bytes, unix_nanos};
pub use logs::{severity_number, to_log_record};
pub use report::{ExportReport, SignalReport, pacing_elapsed};
pub use spans::{GOVERNANCE_SPAN, REQUEST_SPAN, TOOL_SPAN, TraceBatch, to_spans};
pub use transport::{BATCHES_TOTAL, RETRY_DELAYS, is_retryable};


use crate::error::{SchedulerError, SchedulerResult};
use crate::repository::otlp::{
    BATCH_ROWS, OtlpAuditTailRepository, OtlpExportState, OtlpExportStateRepository, SETTLE,
    Watermark,
};
use crate::services::scheduling::job_app_context;

pub const JOB_NAME: &str = "otlp_export";

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

    // Why: without an `observability.otlp` block every tick would be a no-op,
    // so the job is not scheduled at all; the profile is fixed for the life
    // of the process.
    fn configured(&self) -> bool {
        ProfileBootstrap::get()
            .ok()
            .and_then(|profile| profile.observability.otlp())
            .is_some()
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
        let db_pool = ctx.get::<DbPool>()?;
        let pool = db_pool.write_pool();
        let instance_id = job_app_context(ctx)
            .ok()
            .map(|app| app.config().instance_id.clone());

        let report = run(&pool, config, instance_id.as_ref(), true).await?;
        Ok(JobResult::success()
            .with_stats(report.rows(), report.failed())
            .with_duration(start.elapsed().as_millis() as u64))
    }
}

systemprompt_provider_contracts::submit_job!(&OtlpExportJob);

pub async fn export_now(
    pool: &Arc<PgPool>,
    config: &OtlpExportConfig,
    instance_id: Option<&InstanceId>,
) -> SchedulerResult<ExportReport> {
    run(pool, config, instance_id, false).await
}

async fn run(
    pool: &Arc<PgPool>,
    config: &OtlpExportConfig,
    instance_id: Option<&InstanceId>,
    paced: bool,
) -> SchedulerResult<ExportReport> {
    let repository = OtlpExportStateRepository::new(pool.as_ref().clone());
    let tails = OtlpAuditTailRepository::new(pool.as_ref().clone());
    let requests =
        AiRequestRepository::new(&Arc::new(Database::from_pools(Arc::clone(pool), None)));
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
                skipped: 0,
                paced: true,
                error: None,
            });
            continue;
        }
        repository.mark_attempt(signal).await?;
        let stores = SignalStores {
            state: &repository,
            tails: &tails,
            requests: &requests,
        };
        let outcome = export_signal(config, stores, &state, instance_id).await;
        report.signals.push(match outcome {
            Ok(exported) => SignalReport {
                signal,
                rows: exported.rows,
                skipped: exported.skipped,
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
                    skipped: 0,
                    paced: false,
                    error: Some(message),
                }
            },
        });
    }
    Ok(report)
}

#[derive(Debug, Clone, Copy)]
struct SignalStores<'a> {
    state: &'a OtlpExportStateRepository,
    tails: &'a OtlpAuditTailRepository,
    requests: &'a AiRequestRepository,
}

async fn export_signal(
    config: &OtlpExportConfig,
    stores: SignalStores<'_>,
    state: &OtlpExportState,
    instance_id: Option<&InstanceId>,
) -> SchedulerResult<SignalExport> {
    let after = state.watermark();
    let signal =
        OtlpSignal::parse(&state.signal).ok_or_else(|| SchedulerError::UnknownOtlpSignal {
            signal: state.signal.clone(),
        })?;
    let (rows, skipped, next) = match signal {
        OtlpSignal::Traces => {
            let batch = load_trace_batch(stores, &after).await?;
            let next = batch
                .requests
                .last()
                .map(|r| Watermark::new(r.completed_at, r.id.as_str()));
            let rows = batch.requests.len() as u64;
            if next.is_some() {
                let envelope = spans::to_export_request(&batch, instance_id);
                post(config, signal, &envelope).await?;
            }
            (rows, 0, next)
        },
        OtlpSignal::Logs => {
            let tail = stores.tails.list_logs_after(&after, BATCH_ROWS).await?;
            if !tail.rows.is_empty() {
                let envelope = logs::to_export_request(&tail.rows, instance_id);
                post(config, signal, &envelope).await?;
            }
            (tail.rows.len() as u64, tail.skipped, tail.last)
        },
    };
    match next {
        Some(next) => {
            stores.state.advance(signal, &next, rows as i64).await?;
            info!(signal = %signal, rows, skipped, "OTLP batch exported");
        },
        None => stores.state.mark_caught_up(signal, SETTLE).await?,
    }
    Ok(SignalExport { rows, skipped })
}

#[derive(Debug, Clone, Copy)]
struct SignalExport {
    rows: u64,
    skipped: u64,
}

async fn load_trace_batch(
    stores: SignalStores<'_>,
    after: &Watermark,
) -> SchedulerResult<TraceBatch> {
    let tails = stores.tails;
    let mut requests = tails.list_requests_after(after, BATCH_ROWS).await?;
    if requests.is_empty() {
        return Ok(TraceBatch::default());
    }
    let ids: Vec<AiRequestId> = requests.iter().map(|r| r.id.clone()).collect();
    let mut attributions = stores.requests.attributions_for(&ids).await?;
    for request in &mut requests {
        request.attributions = attributions.remove(&request.id).unwrap_or_default();
    }
    let mut batch = TraceBatch {
        requests,
        ..TraceBatch::default()
    };
    batch.ledger = tails.list_ledger_for_requests(&ids).await?;
    let trace_ids = batch.trace_ids();
    if !trace_ids.is_empty() {
        batch.governance = tails.list_governance_for_traces(&trace_ids).await?;
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
        .map_err(|source| SchedulerError::OtlpExport {
            signal,
            source: Box::new(source),
        })
}
