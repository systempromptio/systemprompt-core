//! `cloud fleet`: one command across every tenant in `tenants.json`.
//!
//! `fleet run -- <command>` forwards a read-only command to each hosted
//! tenant over the remote CLI gateway and prefixes every output line with the
//! tenant's name; under `--json` the per-tenant outputs are aggregated into
//! one document. The command is parsed and classified with the same
//! [`CommandDescriptor`](crate::descriptor::CommandDescriptor) the router
//! uses, and anything that is not read-only is refused before any tenant is
//! contacted. `fleet versions` reads each tenant's `/readyz` for its core
//! version and readiness.
//!
//! Managed tenants authenticate with their stored `admin session login`
//! session; self-managed tenants with the operator sign-in, exchanged per
//! tenant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod guard;
mod run;
mod versions;

pub use guard::ensure_read_only;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use systemprompt_cloud::{CloudPath, StoredTenant, TenantStore, get_cloud_paths};

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum FleetCommands {
    #[command(about = "Run a read-only command on every tenant: cloud fleet run -- <command>")]
    Run(FleetRunArgs),

    #[command(about = "Report each tenant's core version and readiness")]
    Versions,
}

#[derive(Debug, Args)]
pub struct FleetRunArgs {
    #[arg(long, default_value_t = 300, help = "Per-tenant timeout in seconds")]
    pub timeout: u64,

    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        required = true,
        help = "The command to run, after --"
    )]
    pub command: Vec<String>,
}

pub async fn execute(cmd: FleetCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        FleetCommands::Run(args) => run::execute(args, &ctx.cli).await,
        FleetCommands::Versions => {
            let result = versions::execute().await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}

fn hosted_tenants() -> Result<Vec<StoredTenant>> {
    let path = get_cloud_paths().resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&path)
        .with_context(|| format!("Failed to load tenants from {}", path.display()))?;
    Ok(store
        .tenants
        .into_iter()
        .filter(|tenant| tenant.provider.is_hosted() && tenant.hostname.is_some())
        .collect())
}
