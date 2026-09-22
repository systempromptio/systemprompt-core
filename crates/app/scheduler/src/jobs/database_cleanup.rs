//! The one retention job: bounded, batched deletes (and body releases) over
//! every high-volume operational table, with the windows read from
//! `profile.retention`.
//!
//! Each table is deleted in batches of `BATCH_ROWS` so no statement takes a
//! long lock and each statement-level capture trigger writes one outbox row
//! per batch instead of one per row. A run stops early at `RUN_BUDGET` and
//! picks up where it left off the next night. Deletion requires `enforce`;
//! without it the job reports what it would delete.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_loader::ServicesBootstrap;
use systemprompt_logging::LoggingRepository;
use systemprompt_models::profile::RetentionConfig;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_ai::repository::ai_requests::ORPHAN_AGE;
use systemprompt_users::UserRepository;
use tracing::{debug, info, warn};

use crate::error::SchedulerError;

const BATCH_ROWS: i64 = 5000;
/// Quota windows are at most a month (plus the carry-forward); a bucket older
/// than this can never be read again.
const QUOTA_BUCKET_DAYS: u32 = 62;
/// `ai.history.retention_days`' own default, used when no services tree is
/// loaded.
const DEFAULT_MESSAGES_DAYS: u32 = 30;
const RUN_BUDGET: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Copy)]
pub struct DatabaseCleanupJob;

/// One table's window and what happened to it this run.
#[derive(Debug, Clone)]
struct RetentionPass {
    table: &'static str,
    days: u32,
    deleted: u64,
    complete: bool,
}

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

/// `(table, days)` in deletion order, message rows first so the request
/// rows they hang off are still present for the cascade-free path.
fn plan(retention: &RetentionConfig) -> Vec<(&'static str, u32)> {
    let messages_days = match retention.ai_request_messages_days {
        Some(days) => days,
        None => match ServicesBootstrap::get() {
            Ok(services) => services.ai.history.retention_days,
            Err(error) => {
                warn!(
                    error = %error,
                    "services config unavailable; ai_request_messages window falls back to {DEFAULT_MESSAGES_DAYS} days"
                );
                DEFAULT_MESSAGES_DAYS
            },
        },
    };
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

/// `table=deleted` pairs, the form the console and `infra jobs` show.
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

async fn delete_orphaned_logs(db_pool: &DbPool, enforce: bool) -> ProviderResult<u64> {
    let logs =
        LoggingRepository::new(db_pool).map_err(|e| ProviderError::Configuration(e.to_string()))?;
    let users =
        UserRepository::new(db_pool).map_err(|e| ProviderError::Configuration(e.to_string()))?;
    let internal =
        |e: systemprompt_logging::models::LoggingError| ProviderError::Internal(e.to_string());
    // Why: `logs` and `users` have different owners, so the orphan set is
    // computed by asking each: the log owners seen, minus the users that
    // still exist.
    let seen = logs.distinct_log_user_ids().await.map_err(internal)?;
    let orphans = users
        .missing_ids(&seen)
        .await
        .map_err(|e| ProviderError::Internal(e.to_string()))?;
    if enforce {
        logs.delete_logs_for_users(&orphans).await.map_err(internal)
    } else {
        let would = logs
            .count_logs_for_users(&orphans)
            .await
            .map_err(internal)?;
        info!(
            would_delete_orphaned_logs = would,
            "enforce disabled: orphaned log rows were not deleted"
        );
        Ok(0)
    }
}

/// Why: `fail_orphaned_pending` already existed but ran only from
/// `journal::recover()`, which fires at boot. A long-lived server therefore
/// never closed a `pending` row whose settlement was lost — three such rows
/// had been open for six days on the 2026-09-22 customer instance. The
/// nightly job gives it a second, regular caller.
async fn fail_orphaned_requests(db_pool: &DbPool, enforce: bool) -> ProviderResult<()> {
    if !enforce {
        info!("enforce disabled: orphaned pending AI requests were not failed");
        return Ok(());
    }
    let requests = AiRequestRepository::new(db_pool)
        .map_err(|e| ProviderError::Configuration(e.to_string()))?;
    let orphans = requests
        .fail_orphaned_pending(ORPHAN_AGE)
        .await
        .map_err(|e| ProviderError::Internal(e.to_string()))?;
    for orphan in &orphans {
        warn!(
            ai_request_id = %orphan.id,
            user_id = %orphan.owner,
            "AI request outlived every receipt; failed with unknown usage"
        );
    }
    Ok(())
}

async fn delete_in_batches(
    pool: &PgPool,
    table: &'static str,
    days: u32,
    cutoff: DateTime<Utc>,
    started: Instant,
) -> ProviderResult<RetentionPass> {
    let mut deleted = 0;
    loop {
        if started.elapsed() > RUN_BUDGET {
            return Ok(RetentionPass {
                table,
                days,
                deleted,
                complete: false,
            });
        }
        let batch = delete_batch(pool, table, cutoff).await?;
        deleted += batch;
        if batch < u64::try_from(BATCH_ROWS).unwrap_or(u64::MAX) {
            return Ok(RetentionPass {
                table,
                days,
                deleted,
                complete: true,
            });
        }
    }
}

async fn delete_batch(pool: &PgPool, table: &str, cutoff: DateTime<Utc>) -> ProviderResult<u64> {
    let result = match table {
        "logs" => {
            sqlx::query!(
                "DELETE FROM logs WHERE ctid = ANY(ARRAY(SELECT ctid FROM logs WHERE timestamp < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "analytics_events" => {
            sqlx::query!(
                "DELETE FROM analytics_events WHERE ctid = ANY(ARRAY(SELECT ctid FROM analytics_events WHERE timestamp < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "ai_request_messages" => {
            sqlx::query!(
                "DELETE FROM ai_request_messages WHERE ctid = ANY(ARRAY(SELECT ctid FROM ai_request_messages WHERE created_at < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "mcp_tool_executions" => {
            sqlx::query!(
                "DELETE FROM mcp_tool_executions WHERE ctid = ANY(ARRAY(SELECT ctid FROM mcp_tool_executions WHERE created_at < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "event_outbox" => {
            sqlx::query!(
                "DELETE FROM event_outbox WHERE ctid = ANY(ARRAY(SELECT ctid FROM event_outbox WHERE created_at < $1 AND (consumer IS NULL OR processed_at IS NOT NULL) LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        // Why: the payload row stays (excerpts, hashes, sizes are the audit
        // trail); only the raw bodies, the bulk of the table, are released.
        "ai_request_payloads" => {
            sqlx::query!(
                "UPDATE ai_request_payloads SET request_body = NULL, response_body = NULL, updated_at = NOW() \
                 WHERE ctid = ANY(ARRAY(SELECT ctid FROM ai_request_payloads WHERE created_at < $1 \
                 AND (request_body IS NOT NULL OR response_body IS NOT NULL) LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "governance_decisions" => {
            sqlx::query!(
                "DELETE FROM governance_decisions WHERE ctid = ANY(ARRAY(SELECT ctid FROM governance_decisions WHERE created_at < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        "ai_quota_buckets" => {
            sqlx::query!(
                "DELETE FROM ai_quota_buckets WHERE ctid = ANY(ARRAY(SELECT ctid FROM ai_quota_buckets WHERE window_start < $1 LIMIT $2))",
                cutoff,
                BATCH_ROWS
            )
            .execute(pool)
            .await
        },
        other => {
            return Err(ProviderError::Configuration(format!(
                "no retention path for table {other}"
            )));
        },
    };
    Ok(result
        .map_err(|e| ProviderError::Internal(e.to_string()))?
        .rows_affected())
}

async fn count_before(pool: &PgPool, table: &str, cutoff: DateTime<Utc>) -> ProviderResult<i64> {
    let count = match table {
        "logs" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM logs WHERE timestamp < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "analytics_events" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM analytics_events WHERE timestamp < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "ai_request_messages" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM ai_request_messages WHERE created_at < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "mcp_tool_executions" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM mcp_tool_executions WHERE created_at < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "event_outbox" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM event_outbox WHERE created_at < $1 AND (consumer IS NULL OR processed_at IS NOT NULL)"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "ai_request_payloads" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM ai_request_payloads WHERE created_at < $1 AND (request_body IS NOT NULL OR response_body IS NOT NULL)"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "governance_decisions" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM governance_decisions WHERE created_at < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        "ai_quota_buckets" => {
            sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!" FROM ai_quota_buckets WHERE window_start < $1"#,
                cutoff
            )
            .fetch_one(pool)
            .await
        },
        other => {
            return Err(ProviderError::Configuration(format!(
                "no retention path for table {other}"
            )));
        },
    };
    count.map_err(|e| ProviderError::Internal(e.to_string()))
}

systemprompt_provider_contracts::submit_job!(&DatabaseCleanupJob);
