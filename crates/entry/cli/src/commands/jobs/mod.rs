use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::Subcommand;
use std::sync::Arc;
use systemprompt_core_analytics::SessionCleanupService;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext};
use tracing_subscriber::EnvFilter;

use systemprompt_generator as _;

#[derive(Debug, Subcommand)]
pub enum JobsCommands {
    #[command(about = "List available jobs")]
    List,
    #[command(about = "Run a scheduled job manually")]
    Run { job_name: String },
    #[command(about = "Clean up inactive sessions")]
    CleanupSessions {
        #[arg(long, default_value = "1")]
        hours: i32,
    },
    #[command(about = "Clean up old log entries")]
    LogCleanup {
        #[arg(long, default_value = "30")]
        days: i32,
    },
    #[command(about = "Clean up inactive sessions (alias)")]
    SessionCleanup {
        #[arg(long, default_value = "1")]
        hours: i32,
    },
}

pub async fn execute(cmd: JobsCommands, config: &CliConfig) -> Result<()> {
    let ctx = Arc::new(AppContext::new().await?);

    match cmd {
        JobsCommands::List => {
            list_jobs(config);
            Ok(())
        },
        JobsCommands::Run { job_name } => run_job(&job_name, ctx).await,
        JobsCommands::CleanupSessions { hours } | JobsCommands::SessionCleanup { hours } => {
            cleanup_sessions(hours, ctx).await
        },
        JobsCommands::LogCleanup { days } => cleanup_logs(days, ctx).await,
    }
}

fn list_jobs(config: &CliConfig) {
    if config.is_json_output() {
        let jobs: Vec<serde_json::Value> = inventory::iter::<&'static dyn Job>
            .into_iter()
            .map(|job| {
                serde_json::json!({
                    "name": job.name(),
                    "description": job.description(),
                    "schedule": job.schedule(),
                    "enabled": job.enabled()
                })
            })
            .collect();
        CliService::json(&jobs);
    } else {
        CliService::section("Available Jobs");

        for job in inventory::iter::<&'static dyn Job> {
            let status = if job.enabled() { "enabled" } else { "disabled" };
            CliService::key_value(
                job.name(),
                &format!("{} | {} | {}", job.description(), job.schedule(), status),
            );
        }
    }
}

#[tracing::instrument(name = "cli_jobs", skip(ctx))]
async fn run_job(job_name: &str, ctx: Arc<AppContext>) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();

    CliService::info(&format!("Running job: {}", job_name));

    let db_pool = Arc::clone(ctx.db_pool());

    let job = inventory::iter::<&'static dyn Job>
        .into_iter()
        .find(|&j| j.name() == job_name)
        .copied();

    let Some(job) = job else {
        CliService::error(&format!("Unknown job: {}", job_name));
        CliService::info("Use 'jobs list' to see available jobs");
        anyhow::bail!("Unknown job: {job_name}");
    };

    let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(db_pool);
    let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(ctx);
    let job_ctx = JobContext::new(db_pool_any, app_context_any);

    match job.execute(&job_ctx).await {
        Ok(result) if result.success => {
            CliService::success("Job completed successfully");
            if let Some(msg) = result.message {
                CliService::info(&format!("  {}", msg));
            }
            Ok(())
        },
        Ok(result) => {
            let msg = result
                .message
                .unwrap_or_else(|| "Unknown error".to_string());
            CliService::error(&format!("Job failed: {}", msg));
            anyhow::bail!("Job failed: {msg}")
        },
        Err(e) => {
            CliService::error(&format!("Job failed: {}", e));
            Err(e)
        },
    }
}

async fn cleanup_sessions(hours: i32, ctx: Arc<AppContext>) -> Result<()> {
    CliService::section("Session Cleanup");

    CliService::info(&format!(
        "Cleaning up sessions inactive for >{} hour(s)...",
        hours
    ));

    let cleanup_service = SessionCleanupService::new(Arc::clone(ctx.db_pool()));
    let closed_count = cleanup_service.cleanup_inactive_sessions(hours).await?;

    CliService::success(&format!("Closed {} inactive session(s)", closed_count));

    Ok(())
}

async fn cleanup_logs(days: i32, ctx: Arc<AppContext>) -> Result<()> {
    CliService::section("Log Cleanup");

    CliService::info(&format!(
        "Cleaning up log entries older than {} day(s)...",
        days
    ));

    run_job("database_cleanup", ctx).await
}
