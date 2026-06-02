use crate::cli_settings::CliConfig;
use crate::commands::core::content::types::{JourneyNode, JourneyOutput};
use crate::shared::CommandOutput;
use anyhow::Result;
use clap::Args;
use systemprompt_content::LinkAnalyticsService;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Copy, Args)]
pub struct JourneyArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,
}

pub async fn execute(args: JourneyArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
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
