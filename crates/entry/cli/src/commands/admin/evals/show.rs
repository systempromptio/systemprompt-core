//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use serde::Serialize;
use systemprompt_identifiers::EvalRunId;

use super::shared::eval_context;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Run id")]
    pub run_id: String,
}

#[derive(Debug, Serialize)]
struct ResultRow {
    id: String,
    ai_request_id: String,
    model: String,
    score: String,
    verdict: &'static str,
    repaired: bool,
    rationale: String,
}

pub(super) async fn execute(args: ShowArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let eval = eval_context(ctx).await?;
    let run_id = EvalRunId::new(args.run_id);
    let run = eval.evaluation.get_run(&run_id).await?;
    let results = eval.evaluation.list_results(&run_id).await?;

    let rows: Vec<ResultRow> = results
        .into_iter()
        .map(|result| ResultRow {
            id: result.id.as_str().to_owned(),
            ai_request_id: result
                .ai_request_id
                .map(|id| id.as_str().to_owned())
                .unwrap_or_default(),
            model: result.model,
            score: result
                .overall_score
                .map(|s| s.to_string())
                .unwrap_or_default(),
            verdict: result.verdict.as_str(),
            repaired: result.repaired,
            rationale: result.rationale.unwrap_or_default(),
        })
        .collect();

    Ok(CommandOutput::table_of(
        vec![
            "id",
            "ai_request_id",
            "model",
            "score",
            "verdict",
            "repaired",
            "rationale",
        ],
        &rows,
    )
    .with_title(format!(
        "Run {} — {} {} (scored {}, failed {})",
        run.id,
        run.kind.as_str(),
        run.status.as_str(),
        run.scored_count,
        run.failed_count
    )))
}
