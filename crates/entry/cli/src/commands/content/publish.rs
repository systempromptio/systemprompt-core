use super::types::{PublishPipelineOutput, StepResult};
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::Result;
use clap::{Args, ValueEnum};
use std::sync::Arc;
use std::time::Instant;
use systemprompt_core_content::ContentIngestionJob;
use systemprompt_generator::{generate_sitemap, prerender_content, prerender_homepage};
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum PublishStep {
    /// Ingest markdown from configured sources
    Ingest,
    /// Generate static HTML pages
    Prerender,
    /// Generate homepage
    Homepage,
    /// Generate sitemap.xml
    Sitemap,
    /// Run all steps (default)
    All,
}

#[derive(Debug, Args)]
pub struct PublishArgs {
    #[arg(long, short = 's', value_enum, help = "Run specific step(s) only")]
    pub step: Option<Vec<PublishStep>>,

    #[arg(long, help = "Skip content ingestion step")]
    pub skip_ingest: bool,

    #[arg(long, help = "Show verbose output")]
    pub verbose: bool,
}

impl PublishArgs {
    fn should_run(&self, step: PublishStep) -> bool {
        self.step.as_ref().map_or_else(
            || {
                if step == PublishStep::Ingest {
                    !self.skip_ingest
                } else {
                    true
                }
            },
            |steps| {
                if steps.contains(&PublishStep::All) {
                    if step == PublishStep::Ingest {
                        !self.skip_ingest
                    } else {
                        true
                    }
                } else {
                    steps.contains(&step)
                }
            },
        )
    }
}

pub async fn execute(
    args: PublishArgs,
    _config: &CliConfig,
) -> Result<CommandResult<PublishPipelineOutput>> {
    let start_time = Instant::now();
    let verbose = args.verbose;

    let ctx = AppContext::new().await?;
    let db_pool = ctx.db_pool();

    let mut steps: Vec<StepResult> = Vec::new();

    if args.should_run(PublishStep::Ingest) {
        let step_start = Instant::now();
        if verbose {
            tracing::info!("Starting content ingestion...");
        }

        let result = ContentIngestionJob::execute_ingestion(db_pool).await;
        let duration_ms = step_start.elapsed().as_millis() as u64;

        steps.push(StepResult {
            step: "ingest".to_string(),
            success: result.is_ok(),
            duration_ms,
            message: result.err().map(|e| e.to_string()),
        });
    }

    if args.should_run(PublishStep::Prerender) {
        let step_start = Instant::now();
        if verbose {
            tracing::info!("Starting content prerendering...");
        }

        let result = prerender_content(Arc::clone(db_pool)).await;
        let duration_ms = step_start.elapsed().as_millis() as u64;

        steps.push(StepResult {
            step: "prerender".to_string(),
            success: result.is_ok(),
            duration_ms,
            message: result.err().map(|e| e.to_string()),
        });
    }

    if args.should_run(PublishStep::Homepage) {
        let step_start = Instant::now();
        if verbose {
            tracing::info!("Starting homepage prerendering...");
        }

        let result = prerender_homepage(Arc::clone(db_pool)).await;
        let duration_ms = step_start.elapsed().as_millis() as u64;

        steps.push(StepResult {
            step: "homepage".to_string(),
            success: result.is_ok(),
            duration_ms,
            message: result.err().map(|e| e.to_string()),
        });
    }

    if args.should_run(PublishStep::Sitemap) {
        let step_start = Instant::now();
        if verbose {
            tracing::info!("Starting sitemap generation...");
        }

        let result = generate_sitemap(Arc::clone(db_pool)).await;
        let duration_ms = step_start.elapsed().as_millis() as u64;

        steps.push(StepResult {
            step: "sitemap".to_string(),
            success: result.is_ok(),
            duration_ms,
            message: result.err().map(|e| e.to_string()),
        });
    }

    let total_duration_ms = start_time.elapsed().as_millis() as u64;
    let succeeded = steps.iter().filter(|s| s.success).count();
    let failed = steps.iter().filter(|s| !s.success).count();
    let total_steps = steps.len();

    let output = PublishPipelineOutput {
        steps,
        total_steps,
        succeeded,
        failed,
        duration_ms: total_duration_ms,
    };

    let title = if failed == 0 {
        "Content Published Successfully"
    } else {
        "Content Publish Completed with Errors"
    };

    Ok(CommandResult::card(output).with_title(title))
}
