//! Classifying a command line without running it.
//!
//! [`describe_command_line`] parses an argv (without the binary name) against
//! the full command tree — kernel groups and every registered group — and
//! returns the descriptor the runner would bootstrap it with, so a command
//! that forwards another command (such as a fleet-wide run) judges it by the
//! router's own classification. [`command_line_path`] returns the
//! subcommand path the same argv selects (`["admin", "session", "show"]`), so
//! such a command can name what it refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::FromArgMatches;

use clap::ArgMatches;

use super::CommandRegistry;
use crate::args::{self, Cli};
use crate::descriptor::{CommandDescriptor, DescribeCommand};

fn parse(command: &[String]) -> Result<(CommandRegistry, ArgMatches)> {
    let argv = std::iter::once("systemprompt".to_owned()).chain(command.iter().cloned());
    let registry = CommandRegistry::collected()?;
    let matches = args::command(&registry)?
        .try_get_matches_from(argv)
        .with_context(|| format!("'{}' is not a systemprompt command", command.join(" ")))?;
    Ok((registry, matches))
}

pub fn command_line_path(command: &[String]) -> Result<Vec<String>> {
    let (_, matches) = parse(command)?;
    let mut path = Vec::new();
    let mut current = &matches;
    while let Some((name, sub)) = current.subcommand() {
        path.push(name.to_owned());
        current = sub;
    }
    Ok(path)
}

pub fn describe_command_line(command: &[String]) -> Result<CommandDescriptor> {
    let (registry, matches) = parse(command)?;
    if let Some(registered) = registry.resolve(&matches)? {
        return Ok(registered.descriptor());
    }
    let parsed = Cli::from_arg_matches(&matches)?
        .command
        .ok_or_else(|| anyhow!("'{}' names no command", command.join(" ")))?;
    Ok(parsed.descriptor())
}
