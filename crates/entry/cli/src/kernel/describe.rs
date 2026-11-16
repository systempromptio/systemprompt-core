//! Classifying a command line without running it.
//!
//! [`describe_command_line`] parses an argv (without the binary name) against
//! the full command tree — kernel groups and every registered group — and
//! returns the descriptor the runner would bootstrap it with, so a command
//! that forwards another command (such as a fleet-wide run) judges it by the
//! router's own classification. [`command_line_path`] returns the
//! subcommand path the same argv selects (`["admin", "session", "show"]`), so
//! such a command can name what it refused. [`classify_command_line`] returns
//! both with the owning member, and refuses a command whose owner the profile
//! disables, exactly as dispatch would.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::{ArgMatches, FromArgMatches};
use systemprompt_extension::cli::{CommandDescriptor, DescribeCommand, DescribedCommand};
use systemprompt_manifest::Profile;

use super::{CommandOwner, CommandRegistry};
use crate::args::{self, Cli};
use crate::runner::refuse_disabled;

fn parse(registry: &CommandRegistry, command: &[String]) -> Result<ArgMatches> {
    let argv = std::iter::once("systemprompt".to_owned()).chain(command.iter().cloned());
    args::command(registry)?
        .try_get_matches_from(argv)
        .with_context(|| format!("'{}' is not a systemprompt command", command.join(" ")))
}

fn path_of(matches: &ArgMatches) -> Vec<String> {
    let mut path = Vec::new();
    let mut current = matches;
    while let Some((name, sub)) = current.subcommand() {
        path.push(name.to_owned());
        current = sub;
    }
    path
}

fn kernel_descriptor(matches: &ArgMatches, command: &[String]) -> Result<CommandDescriptor> {
    let parsed = Cli::from_arg_matches(matches)?
        .command
        .ok_or_else(|| anyhow!("'{}' names no command", command.join(" ")))?;
    Ok(parsed.descriptor())
}

pub fn command_line_path(registry: &CommandRegistry, command: &[String]) -> Result<Vec<String>> {
    Ok(path_of(&parse(registry, command)?))
}

pub fn describe_command_line(
    registry: &CommandRegistry,
    command: &[String],
) -> Result<CommandDescriptor> {
    let matches = parse(registry, command)?;
    if let Some(resolved) = registry.resolve(&matches)? {
        return Ok(resolved.parsed.descriptor());
    }
    kernel_descriptor(&matches, command)
}

pub fn classify_command_line(
    registry: &CommandRegistry,
    profile: &Profile,
    command: &[String],
) -> Result<DescribedCommand> {
    let matches = parse(registry, command)?;
    let path = path_of(&matches);
    if let Some(resolved) = registry.resolve(&matches)? {
        refuse_disabled(&resolved, profile)?;
        let owner = match resolved.owner {
            CommandOwner::Member(owner) => Some(owner),
            CommandOwner::Kernel => None,
        };
        return Ok(DescribedCommand {
            path,
            descriptor: resolved.parsed.descriptor(),
            owner,
        });
    }
    Ok(DescribedCommand {
        path,
        descriptor: kernel_descriptor(&matches, command)?,
        owner: None,
    })
}
