use crate::cli_settings::CliConfig;
use crate::commands::content::types::CampaignAnalyticsOutput;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_content::LinkAnalyticsService;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::CampaignId;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct CampaignArgs {
    #[arg(help = "Campaign ID")]
    pub campaign_id: String,
}

pub async fn execute(
    args: CampaignArgs,
    config: &CliConfig,
) -> Result<CommandResult<CampaignAnalyticsOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: CampaignArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<CampaignAnalyticsOutput>> {
    let service = LinkAnalyticsService::new(pool)?;

    let campaign_id = CampaignId::new(args.campaign_id.clone());
    let performance = service
        .get_campaign_performance(&campaign_id)
        .await?
        .ok_or_else(|| anyhow!("Campaign not found: {}", args.campaign_id))?;

    let output = CampaignAnalyticsOutput {
        campaign_id: performance.campaign_id,
        total_clicks: performance.total_clicks,
        link_count: performance.link_count,
        unique_visitors: performance.unique_visitors.unwrap_or(0),
        conversion_count: performance.conversion_count.unwrap_or(0),
    };

    Ok(CommandResult::card(output).with_title("Campaign Analytics"))
}
