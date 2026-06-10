//! `core content analytics` command group: link, campaign, and journey metrics.
//!
//! Dispatches the [`AnalyticsCommands`] subcommands (clicks, campaign,
//! journey) against the invocation's [`CommandContext`].

pub mod campaign;
pub mod clicks;
pub mod journey;

use crate::context::CommandContext;
use crate::shared::render_result;
use anyhow::{Context, Result};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum AnalyticsCommands {
    #[command(about = "Show click history for a link")]
    Clicks(clicks::ClicksArgs),

    #[command(about = "Show campaign-level analytics")]
    Campaign(campaign::CampaignArgs),

    #[command(about = "Show content navigation graph")]
    Journey(journey::JourneyArgs),
}

pub async fn execute(command: AnalyticsCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        AnalyticsCommands::Clicks(args) => {
            let result = clicks::execute(args, ctx)
                .await
                .context("Failed to get link clicks")?;
            render_result(&result, &ctx.cli);
        },
        AnalyticsCommands::Campaign(args) => {
            let result = campaign::execute(args, ctx)
                .await
                .context("Failed to get campaign analytics")?;
            render_result(&result, &ctx.cli);
        },
        AnalyticsCommands::Journey(args) => {
            let result = journey::execute(args, ctx)
                .await
                .context("Failed to get content journey")?;
            render_result(&result, &ctx.cli);
        },
    }
    Ok(())
}
