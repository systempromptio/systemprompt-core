//! `admin evals promote` — promote an AI request into the golden case set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_evaluation::{NewCaseParams, SampleFilter};
use systemprompt_identifiers::AiRequestId;

use super::shared::eval_context;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct PromoteArgs {
    #[arg(help = "AI request id to promote into the golden case set")]
    pub ai_request_id: String,

    #[arg(long, help = "Case name (defaults to the request id)")]
    pub name: Option<String>,

    #[arg(long, help = "Expected behaviour the judge should score against")]
    pub expectation: Option<String>,

    #[arg(long, value_delimiter = ',', help = "Comma-separated tags")]
    pub tags: Vec<String>,
}

pub(super) async fn execute(args: PromoteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let eval = eval_context(ctx).await?;

    let filter = SampleFilter::with_limit(1).ids(vec![args.ai_request_id.clone()]);
    let sampled = eval
        .evaluation
        .sample(&filter)
        .await?
        .into_iter()
        .next()
        .with_context(|| {
            format!(
                "AI request {} not found or has no completed transcript",
                args.ai_request_id
            )
        })?;

    let prompt = sampled.canonical_prompt();
    let case_id = eval
        .evaluation
        .promote_case(&NewCaseParams {
            name: args.name.unwrap_or_else(|| args.ai_request_id.clone()),
            prompt,
            source_ai_request_id: Some(AiRequestId::new(args.ai_request_id)),
            expectation: args.expectation,
            tags: args.tags,
            created_by: eval.admin_id.clone(),
            prepared_body_sha256: sampled.prepared_body_sha256,
        })
        .await?;

    Ok(CommandOutput::text(format!("Promoted case {case_id}")))
}
