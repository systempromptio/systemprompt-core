use crate::cli_settings::CliConfig;
use crate::commands::core::content::types::{GenerateLinkOutput, UtmParamsOutput};
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::{Args, ValueEnum};
use systemprompt_content::models::{LinkType as DomainLinkType, UtmParams};
use systemprompt_content::services::link::generation::{
    GenerateLinkParams, LinkGenerationService,
};
use systemprompt_identifiers::{CampaignId, ContentId};
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LinkType {
    Redirect,
    Utm,
    Both,
}

impl From<LinkType> for DomainLinkType {
    fn from(lt: LinkType) -> Self {
        match lt {
            LinkType::Redirect => Self::Redirect,
            LinkType::Utm => Self::Utm,
            LinkType::Both => Self::Both,
        }
    }
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
    #[arg(long, help = "Target URL")]
    pub url: String,

    #[arg(long, help = "Campaign ID")]
    pub campaign: Option<String>,

    #[arg(long, help = "Campaign name")]
    pub campaign_name: Option<String>,

    #[arg(long, help = "Source content ID")]
    pub content: Option<String>,

    #[arg(long, help = "UTM source")]
    pub utm_source: Option<String>,

    #[arg(long, help = "UTM medium")]
    pub utm_medium: Option<String>,

    #[arg(long, help = "UTM campaign")]
    pub utm_campaign: Option<String>,

    #[arg(long, help = "UTM term")]
    pub utm_term: Option<String>,

    #[arg(long, help = "UTM content")]
    pub utm_content: Option<String>,

    #[arg(long, value_enum, default_value = "both", help = "Link type")]
    pub link_type: LinkType,
}

const DEFAULT_BASE_URL: &str = "https://systemprompt.io";

pub async fn execute(
    args: GenerateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<GenerateLinkOutput>> {
    if args.url.is_empty() {
        return Err(anyhow!("URL is required"));
    }

    let ctx = AppContext::new().await?;
    let service = LinkGenerationService::new(ctx.db_pool())?;

    let has_utm = args.utm_source.is_some()
        || args.utm_medium.is_some()
        || args.utm_campaign.is_some()
        || args.utm_term.is_some()
        || args.utm_content.is_some();

    let utm_params = if has_utm {
        Some(UtmParams {
            source: args.utm_source.clone(),
            medium: args.utm_medium.clone(),
            campaign: args.utm_campaign.clone(),
            term: args.utm_term.clone(),
            content: args.utm_content.clone(),
        })
    } else {
        None
    };

    let params = GenerateLinkParams {
        target_url: args.url.clone(),
        link_type: args.link_type.into(),
        campaign_id: args.campaign.map(CampaignId::new),
        campaign_name: args.campaign_name,
        source_content_id: args.content.map(ContentId::new),
        source_page: None,
        utm_params: utm_params.clone(),
        link_text: None,
        link_position: None,
        expires_at: None,
    };

    let link = service.generate_link(params).await?;

    let short_url = format!("{}/r/{}", DEFAULT_BASE_URL, link.short_code);
    let full_url = link.get_full_url();

    let utm_output = utm_params.map(|p| UtmParamsOutput {
        source: p.source,
        medium: p.medium,
        campaign: p.campaign,
        term: p.term,
        content: p.content,
    });

    let output = GenerateLinkOutput {
        link_id: link.id,
        short_code: link.short_code,
        short_url,
        target_url: link.target_url,
        full_url,
        link_type: link.link_type,
        utm_params: utm_output,
    };

    Ok(CommandResult::card(output).with_title("Generated Link"))
}
