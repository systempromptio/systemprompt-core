//! `admin licence` command tree: the report-only licence check-in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod status;

use crate::kernel::KernelHost;
use anyhow::Result;
use clap::Subcommand;

use systemprompt_extension::cli::CommandOutput;

#[derive(Clone, Copy, Debug, Subcommand)]
pub enum LicenceCommands {
    #[command(about = "Show whether the licence check-in is enabled and its last outcome")]
    Status,
}

pub async fn execute(cmd: LicenceCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    let pool = systemprompt_database::pool_from_handle(host.db().await?)?;
    match cmd {
        LicenceCommands::Status => status::execute(host.profile()?, &pool).await,
    }
}
