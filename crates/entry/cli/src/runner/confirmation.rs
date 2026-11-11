//! The one confirmation gate for irreversible commands.
//!
//! A command whose exhaustive `DataImpact` classification is `Irreversible`
//! is confirmed here, before it runs, and nowhere else. On a terminal the
//! operator is asked; without one the command refuses unless `--yes` was
//! passed, so a script never hangs on a prompt and never assumes consent.
//! The confirmation belongs to the operator who typed the command. When it is
//! routed to a tenant, the operator has already been asked on this side, and
//! the argv is forwarded exactly as typed: no flag is added, so a tenant
//! binary that predates `--yes` on that command accepts it. The tenant's CLI
//! gateway spawns the command with `SYSTEMPROMPT_CLI_REMOTE` set, and a
//! gateway-spawned execution counts as already confirmed.
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
    pub routed: bool,
}

impl ConfirmationRequest {
    #[must_use]
    pub fn from_matches(matches: &ArgMatches, argv: &[String], profile: Option<String>) -> Self {
        Self {
            command: argv.join(" "),
            target: profile.map(|p| format!("profile '{p}'")),
            yes: assume_yes(matches),
            routed: false,
        }
    }

    #[must_use]
    pub fn routed_by_gateway(self, routed: bool) -> Self {
        Self { routed, ..self }
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
    if !desc.requires_confirmation() || request.yes || request.routed {
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
