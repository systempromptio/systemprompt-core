//! The one retention job: bounded, batched deletes (and body releases) over
//! every high-volume operational table, with the windows read from
//! `profile.retention`. Each [`RetentionTarget`] dispatches to the repository
//! of the crate that owns the table; the scheduler issues no delete itself.
//!
//! Deletion requires `enforce`; without it the job only reports each window
//! and deletes nothing.
//!
//! Tables are deleted message rows first, so the request rows they hang off
//! are still present for the cascade-free path. Quota windows are at most a
//! month plus the carry-forward, so a bucket older than `QUOTA_BUCKET_DAYS`
//! can never be read again.
//!
//! The run also fails orphaned pending AI requests, and purges archived users
//! past `retention.archived_users_days` that are not under legal hold.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod archived_users;
mod orphans;
mod tables;

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_manifest::profile::RetentionConfig;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::{debug, info};

use crate::error::SchedulerResult;
use crate::services::scheduling::job_app_context;

use self::archived_users::purge_archived_users;
use self::orphans::{delete_orphaned_logs, fail_orphaned_requests};
use self::tables::{BATCH_ROWS, RetentionOwners, RetentionPass, delete_in_batches};

const QUOTA_BUCKET_DAYS: u32 = 62;

/// The tables the retention job prunes; each variant dispatches to the
/// repository of the crate that owns the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionTarget {
    Logs,
    AnalyticsEvents,
    AiRequestMessages,
    McpToolExecutions,
    EventOutbox,
    AiRequestPayloads,
    GovernanceDecisions,
    AiQuotaBuckets,
}

impl RetentionTarget {
    #[must_use]
    pub const fn table(self) -> &'static str {
        match self {
            Self::Logs => "logs",
            Self::AnalyticsEvents => "analytics_events",
            Self::AiRequestMessages => "ai_request_messages",
            Self::McpToolExecutions => "mcp_tool_executions",
            Self::EventOutbox => "event_outbox",
            Self::AiRequestPayloads => "ai_request_payloads",
            Self::GovernanceDecisions => "governance_decisions",
            Self::AiQuotaBuckets => "ai_quota_buckets",
        }
    }

    async fn purge(self, owners: &RetentionOwners, cutoff: DateTime<Utc>) -> SchedulerResult<u64> {
        let deleted = match self {
            Self::Logs => owners.logs.purge_before(cutoff, BATCH_ROWS).await?,
            Self::AnalyticsEvents => {
                owners
                    .analytics_events
                    .purge_before(cutoff, BATCH_ROWS)
                    .await?
            },
            Self::AiRequestMessages => {
                owners
                    .requests
                    .purge_messages_before(cutoff, BATCH_ROWS)
                    .await?
            },
            Self::McpToolExecutions => {
                owners
                    .tool_executions
                    .purge_before(cutoff, BATCH_ROWS)
                    .await?
            },
            Self::EventOutbox => {
                owners
                    .outbox
                    .purge_processed_before(cutoff, BATCH_ROWS)
                    .await?
            },
            Self::AiRequestPayloads => {
                owners
                    .payloads
                    .release_bodies_before(cutoff, BATCH_ROWS)
                    .await?
            },
            Self::GovernanceDecisions => owners.decisions.purge_before(cutoff, BATCH_ROWS).await?,
            Self::AiQuotaBuckets => {
                owners
                    .quota_buckets
                    .purge_before(cutoff, BATCH_ROWS)
                    .await?
            },
        };
        Ok(deleted)
    }
}

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
        let db_pool = Arc::clone(ctx.get::<DbPool>()?);
        let app = job_app_context(ctx)?;
        let mut retention = app.config().retention;
        // Why: the parameter predates the profile block and operators still
        // pass it from the CLI; it stays as an override for the logs window.
        if let Some(days) = ctx.get_parameter_parsed::<u32>("log_retention_days")? {
            retention.logs_days = days;
        }
        let owners = RetentionOwners::new(&db_pool, app.config().instance_id.clone());

        debug!("Job started");
        let orphaned = delete_orphaned_logs(&db_pool, ctx.enforce()).await?;
        fail_orphaned_requests(&db_pool, ctx.enforce()).await?;
        let plan = plan(&retention, app.services_state());
        let mut passes = Vec::with_capacity(plan.len());
        let mut total = orphaned;
        for (target, days) in plan {
            let cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
            let pass = if ctx.enforce() {
                delete_in_batches(&owners, target, days, cutoff, start_time).await?
            } else {
                info!(
                    table = target.table(),
                    retention_days = days,
                    "enforce disabled: rows past the window were not deleted"
                );
                RetentionPass {
                    table: target.table(),
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

        let archived =
            purge_archived_users(&db_pool, retention.archived_users_days, ctx.enforce()).await?;
        total += archived;

        let duration_ms = u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX);
        debug!(total_deleted = total, duration_ms, "Job completed");
        Ok(JobResult::success()
            .with_message(format!(
                "{} archived_users={archived}",
                summary(orphaned, &passes)
            ))
            .with_stats(total, 0)
            .with_duration(duration_ms))
    }
}

fn plan(
    retention: &RetentionConfig,
    services: &systemprompt_loader::ServicesState,
) -> Vec<(RetentionTarget, u32)> {
    let messages_days = retention
        .ai_request_messages_days
        .unwrap_or_else(|| services.config().ai.history.retention_days);
    vec![
        (RetentionTarget::Logs, retention.logs_days),
        (
            RetentionTarget::AnalyticsEvents,
            retention.analytics_events_days,
        ),
        (RetentionTarget::AiRequestMessages, messages_days),
        (
            RetentionTarget::McpToolExecutions,
            retention.mcp_tool_executions_days,
        ),
        (
            RetentionTarget::EventOutbox,
            retention.outbox_processed_days,
        ),
        (
            RetentionTarget::AiRequestPayloads,
            retention.ai_request_payload_raw_days,
        ),
        (
            RetentionTarget::GovernanceDecisions,
            retention.governance_decisions_days,
        ),
        (RetentionTarget::AiQuotaBuckets, QUOTA_BUCKET_DAYS),
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
