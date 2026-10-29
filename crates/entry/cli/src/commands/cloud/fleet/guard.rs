//! The read-only gate for `cloud fleet run`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;

use crate::args::{Cli, Commands};
use crate::descriptor::{DataImpact, DescribeCommand, RoutingClass};

pub fn ensure_read_only(command: &[String]) -> Result<()> {
    let argv = std::iter::once("systemprompt".to_owned()).chain(command.iter().cloned());
    let cli = Cli::try_parse_from(argv)
        .with_context(|| format!("'{}' is not a systemprompt command", command.join(" ")))?;
    let parsed = cli
        .command
        .ok_or_else(|| anyhow!("cloud fleet run needs a command after --"))?;
    if matches!(parsed, Commands::Cloud(_)) {
        bail!(
            "cloud commands run on this machine, not on tenants; fleet run forwards tenant \
             commands"
        );
    }
    let descriptor = parsed.descriptor();
    if descriptor.data_impact() == DataImpact::Destructive
        || descriptor.routing_class() != RoutingClass::ReadOnly
    {
        bail!(
            "'{}' is not a read-only command; cloud fleet run forwards read-only commands only",
            command.join(" ")
        );
    }
    Ok(())
}
