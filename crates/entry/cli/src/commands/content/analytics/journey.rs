use crate::cli_settings::CliConfig;
use crate::commands::content::types::{JourneyNode, JourneyOutput};
use crate::shared::CommandResult;
use anyhow::Result;
use clap::Args;
use systemprompt_core_content::LinkAnalyticsService;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct JourneyArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,
}

pub async fn execute(
    args: JourneyArgs,
    _config: &CliConfig,
) -> Result<CommandResult<JourneyOutput>> {
    let ctx = AppContext::new().await?;
    let service = LinkAnalyticsService::new(ctx.db_pool())?;

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

    Ok(CommandResult::table(output)
        .with_title("Content Journey")
        .with_columns(vec![
            "source_content_id".to_string(),
            "target_url".to_string(),
            "click_count".to_string(),
        ]))
}
