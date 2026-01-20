use crate::cli_settings::CliConfig;
use crate::commands::core::content::types::{LinkListOutput, LinkSummary};
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_content::LinkAnalyticsService;
use systemprompt_identifiers::{CampaignId, ContentId};
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter by campaign ID")]
    pub campaign: Option<String>,

    #[arg(long, help = "Filter by source content ID")]
    pub content: Option<String>,
}

pub async fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<LinkListOutput>> {
    if args.campaign.is_none() && args.content.is_none() {
        return Err(anyhow!("Either --campaign or --content must be specified"));
    }

    let ctx = AppContext::new().await?;
    let service = LinkAnalyticsService::new(ctx.db_pool())?;

    let links = if let Some(campaign) = &args.campaign {
        let campaign_id = CampaignId::new(campaign.clone());
        service.get_links_by_campaign(&campaign_id).await?
    } else if let Some(content) = &args.content {
        let content_id = ContentId::new(content.clone());
        service.get_links_by_source_content(&content_id).await?
    } else {
        vec![]
    };

    let total = links.len();
    let summaries: Vec<LinkSummary> = links
        .into_iter()
        .map(|link| LinkSummary {
            id: link.id,
            short_code: link.short_code,
            target_url: link.target_url,
            link_type: link.link_type,
            campaign_name: link.campaign_name,
            click_count: link.click_count.unwrap_or(0),
            created_at: link.created_at,
        })
        .collect();

    let output = LinkListOutput {
        links: summaries,
        total,
    };

    Ok(CommandResult::table(output)
        .with_title("Links")
        .with_columns(vec![
            "id".to_string(),
            "short_code".to_string(),
            "target_url".to_string(),
            "click_count".to_string(),
        ]))
}
