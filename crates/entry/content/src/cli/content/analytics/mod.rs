//! `core content analytics` command group: link, campaign, and journey metrics.
//!
//! Dispatches the [`AnalyticsCommands`] subcommands (clicks, campaign,
//! journey) against the invocation's [`CommandHost`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod campaign;
pub mod clicks;
pub mod journey;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::cli::{CommandHost, CommandOutput};

#[derive(Debug, Subcommand)]
pub enum AnalyticsCommands {
    #[command(about = "Show click history for a link")]
    Clicks(clicks::ClicksArgs),

    #[command(about = "Show campaign-level analytics")]
    Campaign(campaign::CampaignArgs),

    #[command(about = "Show content navigation graph")]
    Journey(journey::JourneyArgs),
}

pub async fn execute(command: AnalyticsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match command {
        AnalyticsCommands::Clicks(args) => {
            clicks::execute(args, host)
                .await
                .context("Failed to get link clicks")
        },
        AnalyticsCommands::Campaign(args) => {
            campaign::execute(args, host)
                .await
                .context("Failed to get campaign analytics")
        },
        AnalyticsCommands::Journey(args) => {
            journey::execute(args, host)
                .await
                .context("Failed to get content journey")
        },
    }
}
