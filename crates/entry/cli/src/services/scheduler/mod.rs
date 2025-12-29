use anyhow::Result;
use clap::Subcommand;
use std::sync::Arc;
use systemprompt_core_analytics::SessionCleanupService;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext};

use systemprompt_generator as _;

#[derive(Subcommand)]
pub enum SchedulerCommands {
    #[command(about = "Run a scheduled job manually")]
    Run { job_name: String },
    #[command(about = "Clean up inactive sessions")]
    CleanupSessions {
        #[arg(long, default_value = "1")]
        hours: i32,
    },
    #[command(about = "List available jobs")]
    List,
}

pub async fn execute(cmd: SchedulerCommands, ctx: Arc<AppContext>) -> Result<()> {
    match cmd {
        SchedulerCommands::Run { job_name } => run_job(&job_name, ctx).await,
        SchedulerCommands::CleanupSessions { hours } => cleanup_sessions(hours, ctx).await,
        SchedulerCommands::List => list_jobs(),
    }
}

fn list_jobs() -> Result<()> {
    CliService::section("Available Jobs");

    for job in inventory::iter::<&'static dyn Job> {
        CliService::info(&format!("  {} - {}", job.name(), job.description()));
    }

    Ok(())
}

#[tracing::instrument(name = "cli_scheduler", skip(ctx))]
async fn run_job(job_name: &str, ctx: Arc<AppContext>) -> Result<()> {
    CliService::info(&format!("Running job: {}", job_name));

    let db_pool = ctx.db_pool().clone();

    let job = inventory::iter::<&'static dyn Job>
        .into_iter()
        .find(|&j| j.name() == job_name)
        .copied();

    let Some(job) = job else {
        CliService::error(&format!("Unknown job: {}", job_name));
        CliService::info("Use 'scheduler list' to see available jobs");
        anyhow::bail!("Unknown job: {job_name}");
    };

    let job_ctx = JobContext::new(db_pool, ctx.clone());

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

    let cleanup_service = SessionCleanupService::new(ctx.db_pool().clone());
    let closed_count = cleanup_service.cleanup_inactive_sessions(hours).await?;

    CliService::success(&format!("Closed {} inactive session(s)", closed_count));

    Ok(())
}
