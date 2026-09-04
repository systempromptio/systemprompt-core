//! `infra logs governance` — the warn-mode read surface.
//!
//! Warn mode records what governance *would* have refused and lets the call
//! through, which is only useful if the recording can be read back. This
//! subcommand is that reader: [`report`] rolls up `governance_decisions` rows
//! with `decision = 'warn'` and the gateway's `ai_safety_findings`, so a block
//! list or an entropy threshold can be retuned from traffic rather than from
//! guesses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod report;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;

#[derive(Debug, Subcommand)]
pub enum GovernanceCommands {
    #[command(
        about = "Roll up governance warnings and safety findings so warn-mode tunables can be \
                 set from traffic",
        after_help = "EXAMPLES:\n  systemprompt infra logs governance report\n  systemprompt \
                      infra logs governance report --since 24h --group-by policy\n  \
                      systemprompt infra logs governance report --group-by user --format csv"
    )]
    Report(report::ReportArgs),
}

pub async fn execute(command: GovernanceCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        GovernanceCommands::Report(args) => report::execute(args, ctx).await,
    }
}
