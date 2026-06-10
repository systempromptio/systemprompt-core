use crate::cli_settings::CliConfig;
use crate::commands::core::content::types::CampaignAnalyticsOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;
use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_content::LinkAnalyticsService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::CampaignId;

#[derive(Debug, Args)]
pub struct CampaignArgs {
    #[arg(help = "Campaign ID")]
    pub campaign_id: String,
}

pub async fn execute(args: CampaignArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub async fn execute_with_pool(
    args: CampaignArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
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

    Ok(CommandOutput::card_value("Campaign Analytics", &output))
}
