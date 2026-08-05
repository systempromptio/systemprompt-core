//! `admin evals run` — start an evaluation run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Args;
use systemprompt_evaluation::SampleFilter;

use super::shared::{eval_context, run_request};
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(
        long,
        default_value_t = 20,
        help = "Number of recent requests to judge"
    )]
    pub sample_size: i64,

    #[arg(long, default_value_t = 24, help = "Sampling window in hours")]
    pub window_hours: i64,

    #[arg(long, help = "Rubric name (defaults to the built-in 'default' rubric)")]
    pub rubric: Option<String>,

    #[arg(
        long,
        help = "Judge provider (defaults to the configured default provider)"
    )]
    pub judge_provider: Option<String>,

    #[arg(long, help = "Judge model (defaults to the provider's default model)")]
    pub judge_model: Option<String>,

    #[arg(long, help = "Only sample requests served by this provider")]
    pub provider: Option<String>,

    #[arg(long, help = "Only sample requests served by this model")]
    pub model: Option<String>,

    #[arg(long, help = "Abort once judge spend reaches this many microdollars")]
    pub budget_microdollars: Option<i64>,
}

pub(super) async fn execute(args: RunArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let eval = eval_context(ctx).await?;

    let mut filter = SampleFilter::with_limit(args.sample_size)
        .since(Utc::now() - Duration::hours(args.window_hours));
    if let Some(provider) = args.provider {
        filter = filter.provider(provider);
    }
    if let Some(model) = args.model {
        filter = filter.model(model);
    }

    let request = run_request(
        &eval,
        super::shared::JudgeOptions {
            judge_provider: args.judge_provider,
            judge_model: args.judge_model,
            rubric: args.rubric,
            budget_microdollars: args.budget_microdollars,
        },
        filter,
    );
    let (run_id, report) = eval.evaluation.run_judge(request).await?;
    Ok(CommandOutput::card_value(
        "Evaluation run",
        &super::shared::report_card(&run_id, report),
    ))
}
