//! The read-only gate for `cloud fleet run`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow, bail};
use clap::FromArgMatches;

use crate::args::{self, Cli, Commands};
use crate::descriptor::{CommandDescriptor, DescribeCommand, RoutingClass};
use crate::kernel::CommandRegistry;

pub fn ensure_read_only(command: &[String]) -> Result<()> {
    let argv = std::iter::once("systemprompt".to_owned()).chain(command.iter().cloned());
    let registry = CommandRegistry::collected()?;
    let matches = args::command(&registry)?
        .try_get_matches_from(argv)
        .with_context(|| format!("'{}' is not a systemprompt command", command.join(" ")))?;
    let descriptor = forwarded_descriptor(&registry, &matches)?;
    if descriptor.data_impact().destroys_data()
        || descriptor.routing_class() != RoutingClass::ReadOnly
    {
        bail!(
            "'{}' is not a read-only command; cloud fleet run forwards read-only commands only",
            command.join(" ")
        );
    }
    Ok(())
}

fn forwarded_descriptor(
    registry: &CommandRegistry,
    matches: &clap::ArgMatches,
) -> Result<CommandDescriptor> {
    if let Some(registered) = registry.resolve(matches)? {
        return Ok(registered.descriptor());
    }
    let parsed = Cli::from_arg_matches(matches)?
        .command
        .ok_or_else(|| anyhow!("cloud fleet run needs a command after --"))?;
    if matches!(parsed, Commands::Cloud(_)) {
        bail!(
            "cloud commands run on this machine, not on tenants; fleet run forwards tenant \
             commands"
        );
    }
    Ok(parsed.descriptor())
}
