//! The parsed invocation and its dispatch.
//!
//! [`Invocation`] holds the global options and the selected command: a kernel
//! command, a group registered with the
//! [`CommandRegistry`](crate::kernel::CommandRegistry), or none. Its
//! descriptor drives bootstrap; [`dispatch_command`] then runs the selection
//! against the built [`CommandContext`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::{ArgMatches, Command, FromArgMatches};

use super::args;
use crate::commands::{admin, analytics, cloud, core, infrastructure, plugins, web};
use crate::context::CommandContext;
use crate::descriptor::{CommandDescriptor, DbUrlRouting, DescribeCommand};
use crate::kernel::{CommandRegistry, RegisteredCommand};

pub(super) struct Invocation {
    pub(super) cli: args::Cli,
    registered: Option<RegisteredCommand>,
    root: Command,
}

impl Invocation {
    pub(super) fn parse(
        registry: &CommandRegistry,
        root: Command,
        matches: &ArgMatches,
    ) -> Result<Self, clap::Error> {
        if let Some(registered) = registry.resolve(matches)? {
            return Ok(Self {
                cli: args::Cli::from_globals(matches)?,
                registered: Some(registered),
                root,
            });
        }
        Ok(Self {
            cli: args::Cli::from_arg_matches(matches)?,
            registered: None,
            root,
        })
    }

    pub(super) fn descriptor(&self) -> CommandDescriptor {
        match (&self.registered, &self.cli.command) {
            (Some(registered), _) => registered.descriptor(),
            (None, Some(command)) => command.descriptor(),
            (None, None) => CommandDescriptor::FULL,
        }
    }

    pub(super) const fn is_bare(&self) -> bool {
        self.registered.is_none() && self.cli.command.is_none()
    }

    pub(super) const fn db_url_routing(&self) -> Option<DbUrlRouting> {
        match (&self.registered, &self.cli.command) {
            (Some(registered), _) => Some(registered.db_url_routing()),
            (None, Some(command)) => Some(command.db_url_routing()),
            (None, None) => None,
        }
    }

    pub(super) fn into_selected(self) -> Selected {
        match (self.registered, self.cli.command) {
            (Some(registered), _) => Selected::Registered(registered),
            (None, Some(command)) => Selected::Kernel(Box::new(command)),
            (None, None) => Selected::Bare(Box::new(self.root)),
        }
    }
}

pub(super) enum Selected {
    Kernel(Box<args::Commands>),
    Registered(RegisteredCommand),
    Bare(Box<Command>),
}

pub(super) async fn dispatch_command(selected: Selected, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped() {
        let accepted = match &selected {
            Selected::Kernel(command) => matches!(
                **command,
                args::Commands::Core(_)
                    | args::Commands::Infra(_)
                    | args::Commands::Admin(_)
                    | args::Commands::Analytics(_)
            ),
            Selected::Registered(registered) => registered.db_url_routing() == DbUrlRouting::Direct,
            Selected::Bare(_) => {
                bail!("No subcommand provided. Use --help to see available commands.")
            },
        };
        if !accepted {
            bail!("This command requires full profile initialization. Remove --database-url flag.");
        }
    }

    match selected {
        Selected::Kernel(command) => match *command {
            args::Commands::Core(cmd) => Box::pin(core::execute(cmd, ctx)).await?,
            args::Commands::Infra(cmd) => infrastructure::execute(cmd, ctx).await?,
            args::Commands::Admin(cmd) => Box::pin(admin::execute(cmd, ctx)).await?,
            args::Commands::Cloud(cmd) => cloud::execute(cmd, ctx).await?,
            args::Commands::Analytics(cmd) => analytics::execute(cmd, ctx).await?,
            args::Commands::Web(cmd) => web::execute(cmd, ctx)?,
            args::Commands::Plugins(cmd) => {
                Box::pin(plugins::execute(cmd, ctx)).await?;
            },
        },
        Selected::Registered(registered) => registered.run(ctx).await?,
        Selected::Bare(root) => {
            (*root).get_matches_from(["systemprompt", "--help"]);
        },
    }

    Ok(())
}
