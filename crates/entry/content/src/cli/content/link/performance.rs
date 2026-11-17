//! `core content link performance` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli::content::types::LinkPerformanceOutput;
use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_content::{ContentRepositories, LinkAnalyticsService};
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_identifiers::LinkId;

#[derive(Debug, Args)]
pub struct PerformanceArgs {
    #[arg(help = "Link ID", value_parser = systemprompt_extension::cli::parsers::parse_link_id)]
    pub link_id: LinkId,
}

pub async fn execute(args: PerformanceArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let repositories = host.resolved().await?.get::<Arc<ContentRepositories>>()?;
    let service = LinkAnalyticsService::new(
        repositories.link.clone(),
        repositories.link_analytics.clone(),
    );

    let performance = service
        .find_link_performance(&args.link_id)
        .await?
        .ok_or_else(|| anyhow!("Link not found: {}", args.link_id))?;

    let output = LinkPerformanceOutput {
        link_id: performance.link_id,
        click_count: performance.click_count,
        unique_click_count: performance.unique_click_count,
        conversion_count: performance.conversion_count,
        conversion_rate: performance.conversion_rate.unwrap_or(0.0),
    };

    Ok(CommandOutput::card_value("Link Performance", &output))
}
