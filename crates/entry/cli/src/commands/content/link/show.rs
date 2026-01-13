use crate::cli_settings::CliConfig;
use crate::commands::content::types::LinkDetailOutput;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_content::services::LinkGenerationService;
use systemprompt_runtime::AppContext;

const DEFAULT_BASE_URL: &str = "https://systemprompt.io";

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Short code")]
    pub short_code: String,
}

pub async fn execute(
    args: ShowArgs,
    _config: &CliConfig,
) -> Result<CommandResult<LinkDetailOutput>> {
    let ctx = AppContext::new().await?;
    let service = LinkGenerationService::new(ctx.db_pool())?;

    let link = service
        .get_link_by_short_code(&args.short_code)
        .await?
        .ok_or_else(|| anyhow!("Link not found: {}", args.short_code))?;

    let full_url = link.get_full_url();
    let short_url = format!("{}/r/{}", DEFAULT_BASE_URL, link.short_code);

    let output = LinkDetailOutput {
        id: link.id,
        short_code: link.short_code,
        target_url: link.target_url,
        full_url: if full_url != short_url {
            full_url
        } else {
            short_url
        },
        link_type: link.link_type,
        campaign_id: link.campaign_id,
        campaign_name: link.campaign_name,
        source_content_id: link.source_content_id,
        click_count: link.click_count.unwrap_or(0),
        unique_click_count: link.unique_click_count.unwrap_or(0),
        conversion_count: link.conversion_count.unwrap_or(0),
        is_active: link.is_active.unwrap_or(true),
        created_at: link.created_at,
    };

    Ok(CommandResult::card(output).with_title("Link Details"))
}
