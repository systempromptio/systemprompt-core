//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::commands::core::content::types::LinkPerformanceOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;
use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_content::LinkAnalyticsService;
use systemprompt_identifiers::LinkId;

#[derive(Debug, Args)]
pub struct PerformanceArgs {
    #[arg(help = "Link ID")]
    pub link_id: String,
}

pub async fn execute(args: PerformanceArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let service = LinkAnalyticsService::new(&pool)?;

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

    Ok(CommandOutput::card_value("Link Performance", &output))
}
