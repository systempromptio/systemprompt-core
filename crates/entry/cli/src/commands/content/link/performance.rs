use crate::cli_settings::CliConfig;
use crate::commands::content::types::LinkPerformanceOutput;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_content::LinkAnalyticsService;
use systemprompt_identifiers::LinkId;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct PerformanceArgs {
    #[arg(help = "Link ID")]
    pub link_id: String,
}

pub async fn execute(
    args: PerformanceArgs,
    _config: &CliConfig,
) -> Result<CommandResult<LinkPerformanceOutput>> {
    let ctx = AppContext::new().await?;
    let service = LinkAnalyticsService::new(ctx.db_pool())?;

    let link_id = LinkId::new(args.link_id.clone());
    let performance = service
        .get_link_performance(&link_id)
        .await?
        .ok_or_else(|| anyhow!("Link not found: {}", args.link_id))?;

    let output = LinkPerformanceOutput {
        link_id: performance.link_id,
        click_count: performance.click_count,
        unique_click_count: performance.unique_click_count,
        conversion_count: performance.conversion_count,
        conversion_rate: performance.conversion_rate.unwrap_or(0.0),
    };

    Ok(CommandResult::card(output).with_title("Link Performance"))
}
