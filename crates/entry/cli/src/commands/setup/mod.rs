mod docker;
mod postgres;
mod profile;
mod secrets;
mod wizard;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct SetupArgs {
    /// Target environment (dev, staging, prod)
    #[arg(short, long)]
    pub environment: Option<String>,
}

pub async fn execute(args: SetupArgs) -> Result<()> {
    wizard::execute(args.environment).await
}
