//! Reporting projection operations against the primary database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Subcommand;
use systemprompt_runtime::reporting;
use systemprompt_runtime::reporting::RebuildOutcome;

use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum ProjectionCommands {
    #[command(about = "Show projection generation and pending delivery backlog")]
    Status,
    #[command(about = "Atomically rebuild reporting tables from owner snapshots")]
    Rebuild,
    #[command(about = "Apply up to the requested number of pending reporting facts")]
    Sync {
        #[arg(long, default_value_t = 10000)]
        limit: usize,
    },
}

pub async fn execute(command: ProjectionCommands, ctx: &CommandContext) -> Result<()> {
    let database = ctx.database().await?;
    let pool = database.db_pool();
    match command {
        ProjectionCommands::Status => {},
        ProjectionCommands::Rebuild => {
            // Why: a forced rebuild can be superseded by another replica that
            // owns the generation, in which case nothing was rebuilt here. The
            // status card below reads the same either way, so the outcome has
            // to be said out loud or the operator reads someone else's rebuild
            // as their own.
            let outcome = reporting::rebuild(pool).await?;
            if outcome != RebuildOutcome::Rebuilt {
                render_result(
                    &CommandOutput::card_value(
                        "Reporting projection",
                        &format!("not rebuilt here: {outcome:?}"),
                    ),
                    &ctx.cli,
                );
            }
        },
        ProjectionCommands::Sync { limit } => {
            reporting::process_pending(pool, limit).await?;
        },
    }
    let status = reporting::status(pool).await?;
    render_result(
        &CommandOutput::card_value("Reporting projection", &status),
        &ctx.cli,
    );
    Ok(())
}
