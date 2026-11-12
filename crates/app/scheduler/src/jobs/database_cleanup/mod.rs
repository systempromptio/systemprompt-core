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
//! An extension's retention hold (`register_retention_hold!`) clamps a
//! target's cutoff to the earliest watermark it reports, and a hold that
//! cannot answer withholds the target for the run; the pass line and the job
//! message name the hold either way.
//!
//! The run also fails orphaned pending AI requests, and purges archived users
//! past `retention.archived_users_days` that are not under legal hold.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod archived_users;
mod holds;
mod orphans;
mod tables;

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use systemprompt_database::DbPool;
pub use systemprompt_extension::RetentionTarget;
use systemprompt_manifest::profile::RetentionConfig;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::debug;

use crate::services::scheduling::job_app_context;

use self::archived_users::purge_archived_users;
use self::holds::{HoldOutcome, retention_holds};
use self::orphans::{delete_orphaned_logs, fail_orphaned_requests};
use self::tables::{PassRunner, RetentionOwners, RetentionPass};

const QUOTA_BUCKET_DAYS: u32 = 62;

#[derive(Debug, Clone, Copy)]
pub struct DatabaseCleanupJob;

#[async_trait]
impl Job for DatabaseCleanupJob {
    fn name(&self) -> &'static str {
        "database_cleanup"
    }

    fn description(&self) -> &'static str {
        "Deletes logs, analytics events, stored AI request messages, MCP tool executions, processed outbox rows, engagement events and idle fingerprint reputations past their profile.retention windows, in batches; deletion requires enforce"
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
        let holds = retention_holds(ctx, app);
        let mut passes = Vec::with_capacity(plan.len());
        let mut total = orphaned;
        let runner = PassRunner {
            owners: &owners,
            holds: &holds,
            enforce: ctx.enforce(),
            started: start_time,
        };
        for (target, days) in plan {
            let pass = runner.run(target, days).await?;
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
            .with_stats(total, withheld(&passes))
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
        (
            RetentionTarget::EngagementEvents,
            retention.engagement_events_days,
        ),
        (
            RetentionTarget::FingerprintReputation,
            retention.fingerprint_reputation_days,
        ),
    ]
}

fn withheld(passes: &[RetentionPass]) -> u64 {
    let count = passes
        .iter()
        .filter(|pass| matches!(pass.hold, HoldOutcome::Withheld(_)))
        .count();
    u64::try_from(count).unwrap_or(u64::MAX)
}

fn summary(orphaned: u64, passes: &[RetentionPass]) -> String {
    let mut parts = vec![format!("orphaned_logs={orphaned}")];
    parts.extend(passes.iter().map(|pass| {
        let held = match pass.hold {
            HoldOutcome::None => String::new(),
            HoldOutcome::Clamped(hold) => format!("@held:{hold}"),
            HoldOutcome::Withheld(hold) => return format!("{}=withheld:{hold}", pass.table),
        };
        format!(
            "{}={}{}{held}",
            pass.table,
            pass.deleted,
            if pass.complete { "" } else { "+" }
        )
    }));
    parts.join(" ")
}

systemprompt_provider_contracts::submit_job!(&DatabaseCleanupJob);
