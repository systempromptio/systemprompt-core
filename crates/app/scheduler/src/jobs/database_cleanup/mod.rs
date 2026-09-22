//! The one retention job: bounded, batched deletes (and body releases) over
//! every high-volume operational table, with the windows read from
//! `profile.retention`.
//!
//! Deletion requires `enforce`; without it the job reports what it would
//! delete.
//!
//! Tables are deleted message rows first, so the request rows they hang off
//! are still present for the cascade-free path. Quota windows are at most a
//! month plus the carry-forward, so a bucket older than `QUOTA_BUCKET_DAYS`
//! can never be read again; `DEFAULT_MESSAGES_DAYS` mirrors
//! `ai.history.retention_days`' own default for when no services tree is
//! loaded.
//!
//! The run also fails orphaned pending AI requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod orphans;
mod tables;

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_loader::ServicesBootstrap;
use systemprompt_models::profile::RetentionConfig;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};
use tracing::{debug, info, warn};

use crate::error::SchedulerError;

use self::orphans::{delete_orphaned_logs, fail_orphaned_requests};
use self::tables::{RetentionPass, count_before, delete_in_batches};

const QUOTA_BUCKET_DAYS: u32 = 62;
const DEFAULT_MESSAGES_DAYS: u32 = 30;

#[derive(Debug, Clone, Copy)]
pub struct DatabaseCleanupJob;

#[async_trait]
impl Job for DatabaseCleanupJob {
    fn name(&self) -> &'static str {
        "database_cleanup"
    }

    fn description(&self) -> &'static str {
        "Deletes logs, analytics events, stored AI request messages, MCP tool executions and processed outbox rows past their profile.retention windows, in batches; deletion requires enforce"
    }

    fn schedule(&self) -> &'static str {
        "0 0 3 * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = Instant::now();
        let db_pool = Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| SchedulerError::missing_context("DbPool"))?,
        );
        let app = ctx
            .app_context::<Arc<AppContext>>()
            .ok_or_else(|| SchedulerError::missing_context("AppContext"))?;
        let mut retention = app.config().retention;
        // Why: the parameter predates the profile block and operators still
        // pass it from the CLI; it stays as an override for the logs window.
        if let Some(days) = ctx.get_parameter_parsed::<u32>("log_retention_days")? {
            retention.logs_days = days;
        }
        let pool = db_pool
            .write_pool_arc()
            .map_err(|e| ProviderError::Configuration(e.to_string()))?;

        debug!("Job started");
        let orphaned = delete_orphaned_logs(&db_pool, ctx.enforce()).await?;
        fail_orphaned_requests(&db_pool, ctx.enforce()).await?;
        let plan = plan(&retention);
        let mut passes = Vec::with_capacity(plan.len());
        let mut total = orphaned;
        for (table, days) in plan {
            let cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
            let pass = if ctx.enforce() {
                delete_in_batches(&pool, table, days, cutoff, start_time).await?
            } else {
                let would = count_before(&pool, table, cutoff).await?;
                info!(
                    table,
                    would_delete = would,
                    retention_days = days,
                    "enforce disabled: rows qualify for deletion but were not deleted"
                );
                RetentionPass {
                    table,
                    days,
                    deleted: 0,
                    complete: true,
                }
            };
            info!(
                table = pass.table,
                deleted = pass.deleted,
                retention_days = pass.days,
                complete = pass.complete,
                "Retention pass"
            );
            total += pass.deleted;
            passes.push(pass);
        }

        let duration_ms = u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX);
        debug!(total_deleted = total, duration_ms, "Job completed");
        Ok(JobResult::success()
            .with_message(summary(orphaned, &passes))
            .with_stats(total, 0)
            .with_duration(duration_ms))
    }
}

fn services_messages_days() -> u32 {
    match ServicesBootstrap::get() {
        Ok(services) => services.ai.history.retention_days,
        Err(error) => {
            warn!(
                error = %error,
                fallback_days = DEFAULT_MESSAGES_DAYS,
                "services config unavailable; ai_request_messages window falls back to its default"
            );
            DEFAULT_MESSAGES_DAYS
        },
    }
}

fn plan(retention: &RetentionConfig) -> Vec<(&'static str, u32)> {
    let messages_days = retention
        .ai_request_messages_days
        .unwrap_or_else(services_messages_days);
    vec![
        ("logs", retention.logs_days),
        ("analytics_events", retention.analytics_events_days),
        ("ai_request_messages", messages_days),
        ("mcp_tool_executions", retention.mcp_tool_executions_days),
        ("event_outbox", retention.outbox_processed_days),
        ("ai_request_payloads", retention.ai_request_payload_raw_days),
        ("governance_decisions", retention.governance_decisions_days),
        ("ai_quota_buckets", QUOTA_BUCKET_DAYS),
    ]
}

fn summary(orphaned: u64, passes: &[RetentionPass]) -> String {
    let mut parts = vec![format!("orphaned_logs={orphaned}")];
    parts.extend(passes.iter().map(|pass| {
        format!(
            "{}={}{}",
            pass.table,
            pass.deleted,
            if pass.complete { "" } else { "+" }
        )
    }));
    parts.join(" ")
}

systemprompt_provider_contracts::submit_job!(&DatabaseCleanupJob);
