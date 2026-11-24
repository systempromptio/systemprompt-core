//! The parsed invocation and its dispatch.
//!
//! [`Invocation`] holds the global options and the selected command: a kernel
//! command, a group registered with the
//! [`CommandRegistry`](crate::kernel::CommandRegistry), or none. Its
//! descriptor drives bootstrap; [`dispatch_command`] then runs the selection
//! against the built [`CommandContext`] and renders the output once: the
//! output a command returns, or the output a failed command's
//! [`CommandError`] carries, rendered before the failure is reported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::{ArgMatches, Command, FromArgMatches};

use super::args;
use crate::commands::{admin, core, infrastructure, plugins, web};
use crate::context::CommandContext;
use systemprompt_extension::cli::{
    CommandDescriptor, CommandError, CommandHost, DbUrlRouting, DescribeCommand,
};

use crate::kernel::{CommandRegistry, KernelHost, ResolveError, ResolvedCommand};
use crate::member_host::MemberHost;

pub(super) struct Invocation {
    pub(super) cli: args::Cli,
    registered: Option<ResolvedCommand>,
    root: Command,
}

impl Invocation {
    pub(super) fn parse(
        registry: &CommandRegistry,
        root: Command,
        matches: &ArgMatches,
    ) -> Result<Self, ResolveError> {
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
            (Some(registered), _) => registered.parsed.descriptor(),
            (None, Some(command)) => command.descriptor(),
            (None, None) => CommandDescriptor::FULL,
        }
    }

    pub(super) const fn is_bare(&self) -> bool {
        self.registered.is_none() && self.cli.command.is_none()
    }

    pub(super) const fn db_url_routing(&self) -> Option<DbUrlRouting> {
        match (&self.registered, &self.cli.command) {
            (Some(registered), _) => Some(registered.parsed.db_url_routing()),
            (None, Some(command)) => Some(command.db_url_routing()),
            (None, None) => None,
        }
    }

    pub(super) const fn resolved(&self) -> Option<&ResolvedCommand> {
        self.registered.as_ref()
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
    Registered(ResolvedCommand),
    Bare(Box<Command>),
}

impl Selected {
    pub(super) const fn registered(&self) -> Option<&ResolvedCommand> {
        match self {
            Self::Registered(resolved) => Some(resolved),
            Self::Kernel(_) | Self::Bare(_) => None,
        }
    }
}

pub(super) async fn dispatch_command(selected: Selected, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped() {
        let accepted = match &selected {
            Selected::Kernel(command) => matches!(
                **command,
                args::Commands::Core(_) | args::Commands::Infra(_) | args::Commands::Admin(_)
            ),
            Selected::Registered(registered) => {
                registered.parsed.db_url_routing() == DbUrlRouting::Direct
            },
            Selected::Bare(_) => {
                bail!("No subcommand provided. Use --help to see available commands.")
            },
        };
        if !accepted {
            bail!("This command requires full profile initialization. Remove --database-url flag.");
        }
    }

    let host: &dyn KernelHost = ctx;
    let outcome = match selected {
        Selected::Kernel(command) => match *command {
            args::Commands::Core(cmd) => Box::pin(core::execute(cmd, host)).await,
            args::Commands::Infra(cmd) => Box::pin(infrastructure::execute(cmd, host)).await,
            args::Commands::Admin(cmd) => Box::pin(admin::execute(*cmd, host)).await,
            args::Commands::Web(cmd) => web::execute(cmd, host),
            args::Commands::Plugins(cmd) => Box::pin(plugins::execute(cmd, host)).await,
        },
        Selected::Registered(registered)
            if admin::plugin_token::is_issue_plugin_token(&registered) =>
        {
            Box::pin(admin::plugin_token::run_resolved(&registered, host)).await
        },
        Selected::Registered(registered) => {
            let member = MemberHost::new(ctx, registered.parsed.descriptor());
            let member = match registered.member_scope() {
                Some((owner, standing)) => member.owned_by(owner.clone(), standing),
                None => member,
            };
            registered
                .parsed
                .run(&member)
                .await
                .map_err(anyhow::Error::from)
        },
        Selected::Bare(root) => {
            (*root).get_matches_from(["systemprompt", "--help"]);
            return Ok(());
        },
    };

    match outcome {
        Ok(output) => {
            ctx.render(&output);
            Ok(())
        },
        Err(error) => {
            if let Some(output) = error
                .downcast_ref::<CommandError>()
                .and_then(CommandError::output)
            {
                ctx.render(output);
            }
            Err(error)
        },
    }
}
