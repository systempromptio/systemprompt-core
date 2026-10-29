//! `admin licence` command tree: the report-only licence check-in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod status;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Clone, Copy, Debug, Subcommand)]
pub enum LicenceCommands {
    #[command(about = "Show whether the licence check-in is enabled and its last outcome")]
    Status,
}

pub async fn execute(cmd: LicenceCommands, ctx: &CommandContext) -> Result<()> {
    let app = ctx.app_context().await?;
    let output = match cmd {
        LicenceCommands::Status => status::execute(app).await?,
    };
    render_result(&output, &ctx.cli);
    Ok(())
}
