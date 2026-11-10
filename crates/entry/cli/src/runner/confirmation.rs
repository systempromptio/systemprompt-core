//! The one confirmation gate for irreversible commands.
//!
//! A command whose exhaustive `DataImpact` classification is `Irreversible`
//! is confirmed here, before it runs, and nowhere else. On a terminal the
//! operator is asked; without one the command refuses unless `--yes` was
//! passed, so a script never hangs on a prompt and never assumes consent.
//! A command routed to a remote tenant carries the operator's answer as
//! `--yes`, because the remote side has no terminal to ask on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::ArgMatches;

use crate::cli_settings::CliConfig;
use crate::descriptor::CommandDescriptor;
use crate::interactive::{DialoguerPrompter, Prompter};

const YES_ARG: &str = "yes";

#[derive(Debug, Clone)]
pub struct ConfirmationRequest {
    pub command: String,
    pub target: Option<String>,
    pub yes: bool,
}

impl ConfirmationRequest {
    #[must_use]
    pub fn from_matches(matches: &ArgMatches, argv: &[String], profile: Option<String>) -> Self {
        Self {
            command: argv.join(" "),
            target: profile.map(|p| format!("profile '{p}'")),
            yes: assume_yes(matches),
        }
    }

    #[must_use]
    pub fn for_profile(&self, profile: &str) -> Self {
        Self {
            target: Some(format!("profile '{profile}'")),
            ..self.clone()
        }
    }
}

pub fn gate(
    desc: &CommandDescriptor,
    request: &ConfirmationRequest,
    config: &CliConfig,
) -> Result<()> {
    confirm_irreversible(desc, request, config, &DialoguerPrompter)
}

#[must_use]
pub fn assume_yes(matches: &ArgMatches) -> bool {
    let mut leaf = matches;
    while let Some((_, sub)) = leaf.subcommand() {
        leaf = sub;
    }
    matches!(leaf.try_get_one::<bool>(YES_ARG), Ok(Some(true)))
}

pub fn confirm_irreversible(
    desc: &CommandDescriptor,
    request: &ConfirmationRequest,
    config: &CliConfig,
    prompter: &dyn Prompter,
) -> Result<()> {
    if !desc.requires_confirmation() || request.yes {
        return Ok(());
    }
    let target = request.target.as_deref().unwrap_or("the active profile");
    if !config.is_interactive() {
        bail!(
            "`systemprompt {}` cannot be undone and needs confirmation, but there is no terminal \
             to ask on. Pass --yes to run it against {target}.",
            request.command
        );
    }
    let message = format!(
        "`systemprompt {}` cannot be undone. Run it against {target}?",
        request.command
    );
    if prompter.confirm(&message, false)? {
        Ok(())
    } else {
        bail!("Operation cancelled; nothing was changed")
    }
}

#[must_use]
pub fn forward_with_yes(desc: &CommandDescriptor, mut args: Vec<String>) -> Vec<String> {
    if !desc.requires_confirmation() || args.iter().any(|a| a == "--yes" || a == "-y") {
        return args;
    }
    let at = args.iter().position(|a| a == "--").unwrap_or(args.len());
    args.insert(at, "--yes".to_owned());
    args
}
