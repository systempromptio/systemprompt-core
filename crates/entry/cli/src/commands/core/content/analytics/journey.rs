//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli_settings::CliConfig;
use crate::commands::core::content::types::{JourneyNode, JourneyOutput};
use crate::context::CommandContext;
use crate::shared::CommandOutput;
use anyhow::Result;
use clap::Args;
use systemprompt_content::LinkAnalyticsService;
use systemprompt_database::DbPool;

#[derive(Debug, Clone, Copy, Args)]
pub struct JourneyArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,
}

pub async fn execute(args: JourneyArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub async fn execute_with_pool(
    args: JourneyArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let service = LinkAnalyticsService::new(pool)?;

    let nodes = service
        .get_content_journey_map(Some(args.limit), Some(args.offset))
        .await?;

    let journey_nodes: Vec<JourneyNode> = nodes
        .into_iter()
        .map(|node| JourneyNode {
            source_content_id: node.source_content_id,
            target_url: node.target_url,
            click_count: node.click_count,
        })
        .collect();

    let output = JourneyOutput {
        nodes: journey_nodes,
    };

    Ok(CommandOutput::table_of(
        vec!["source_content_id", "target_url", "click_count"],
        &output.nodes,
    )
    .with_title("Content Journey"))
}
