//! `admin evals replay` — replay a run's failures against the provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use systemprompt_evaluation::SampleFilter;
use systemprompt_identifiers::EvalRunId;

use super::shared::{eval_context, run_request};
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ReplayArgs {
    #[arg(help = "Source run id whose failures should be replayed")]
    pub run_id: String,

    #[arg(long, help = "Rubric name (defaults to the built-in 'default' rubric)")]
    pub rubric: Option<String>,

    #[arg(
        long,
        help = "Judge provider (defaults to the configured default provider)"
    )]
    pub judge_provider: Option<String>,

    #[arg(long, help = "Judge model (defaults to the provider's default model)")]
    pub judge_model: Option<String>,

    #[arg(long, help = "Abort once judge spend reaches this many microdollars")]
    pub budget_microdollars: Option<i64>,
}

pub async fn execute(args: ReplayArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let eval = eval_context(ctx).await?;
    let source_run = EvalRunId::new(args.run_id);

    let request = run_request(
        &eval,
        super::shared::JudgeOptions {
            judge_provider: args.judge_provider,
            judge_model: args.judge_model,
            rubric: args.rubric,
            budget_microdollars: args.budget_microdollars,
        },
        SampleFilter::default(),
    );
    let (run_id, report) = eval
        .evaluation
        .replay_failures(&source_run, request)
        .await?;
    Ok(CommandOutput::card_value(
        "Replay run",
        &super::shared::report_card(&run_id, report),
    ))
}
