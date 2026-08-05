//! `admin evals list` — list evaluation runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use serde::Serialize;

use super::shared::eval_context;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, default_value_t = 20, help = "Maximum runs to show")]
    pub limit: i64,
}

#[derive(Debug, Serialize)]
struct RunRow {
    id: String,
    kind: &'static str,
    status: &'static str,
    judge: String,
    scored: i32,
    failed: i32,
    cost_microdollars: i64,
    created_at: String,
}

pub(super) async fn execute(args: ListArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let eval = eval_context(ctx).await?;
    let runs = eval.evaluation.list_runs(args.limit).await?;

    let rows: Vec<RunRow> = runs
        .into_iter()
        .map(|run| RunRow {
            id: run.id.as_str().to_owned(),
            kind: run.kind.as_str(),
            status: run.status.as_str(),
            judge: format!("{}/{}", run.judge_provider, run.judge_model),
            scored: run.scored_count,
            failed: run.failed_count,
            cost_microdollars: run.cost_microdollars,
            created_at: run.created_at.to_rfc3339(),
        })
        .collect();

    Ok(CommandOutput::table_of(
        vec![
            "id",
            "kind",
            "status",
            "judge",
            "scored",
            "failed",
            "cost_microdollars",
            "created_at",
        ],
        &rows,
    ))
}
