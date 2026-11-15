//! The traits a contributed command implements, and its type-erased form.
//!
//! [`CliCommandGroup`] (a clap `Subcommand` enum) and [`CliCommandLeaf`] (a
//! clap `Args` struct) declare where they attach, their help and their
//! [`DescribeCommand`] descriptor. Once parsed, either becomes a
//! [`RegisteredCommand`]: its descriptor, its `--database-url` routing and a
//! `Send` future that runs it with the kernel [`CommandContext`].
//!
//! `DISPLAY_ORDER` places a top-level group in `--help`. The kernel's own
//! groups hold the slots `10` (`core`), `20` (`infra`), `30` (`admin`), `60`
//! (`web`) and `70` (`plugins`); `40` and `50` are the `cloud` and `analytics`
//! slots. A registration that declares no slot gets
//! [`REGISTERED_DISPLAY_ORDER`], after every kernel group, and a group nested
//! under a kernel group is listed after that group's own subcommands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;

use clap::{ArgMatches, Args, Command, Subcommand};

use super::grants::KernelGrant;
use crate::context::CommandContext;
use crate::descriptor::{CommandDescriptor, DbUrlRouting, DescribeCommand};

pub type CommandFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

pub const REGISTERED_DISPLAY_ORDER: usize = 1000;

type RunFn = for<'a> fn(&'a ArgMatches, &'a CommandContext) -> CommandFuture<'a>;

/// A command group contributed to the CLI from outside the kernel's own tree.
///
/// `PARENT` is the path of the kernel group the group attaches under (empty
/// for a top-level domain), `NAME` its subcommand name and `ABOUT` its
/// one-line help, `DISPLAY_ORDER` its `--help` slot (see the module docs).
/// [`DescribeCommand::descriptor`] returns the descriptor for the parsed
/// command, data impact included; `GRANTS` names the [`KernelGrant`]s the
/// group needs, which the kernel adds to every descriptor it resolves for the
/// group once its closed table lists the group's path for each.
pub trait CliCommandGroup: Subcommand + DescribeCommand + Send + Sized + 'static {
    const PARENT: &'static [&'static str];
    const NAME: &'static str;
    const ABOUT: &'static str;
    const DISPLAY_ORDER: usize = REGISTERED_DISPLAY_ORDER;
    const GRANTS: &'static [KernelGrant] = &[];

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Unsupported
    }

    fn run(self, ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>> + Send;
}

/// A single command contributed to the CLI: one leaf with its own arguments
/// rather than a group of subcommands.
///
/// It attaches under `PARENT` exactly as a [`CliCommandGroup`] does, so a
/// leaf such as `infra services serve` can be contributed by the crate that
/// owns what it runs. `GRANTS` works as for a group and belongs to the leaf
/// alone: a leaf never inherits its parent group's descriptor or grants.
pub trait CliCommandLeaf: Args + DescribeCommand + Send + Sized + 'static {
    const PARENT: &'static [&'static str];
    const NAME: &'static str;
    const ABOUT: &'static str;
    const DISPLAY_ORDER: usize = REGISTERED_DISPLAY_ORDER;
    const GRANTS: &'static [KernelGrant] = &[];

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Unsupported
    }

    fn run(self, ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>> + Send;
}

/// A parsed invocation of a registered group, ready to run.
#[derive(Clone)]
pub struct RegisteredCommand {
    descriptor: CommandDescriptor,
    db_url_routing: DbUrlRouting,
    matches: ArgMatches,
    run: RunFn,
}

impl std::fmt::Debug for RegisteredCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredCommand")
            .field("descriptor", &self.descriptor)
            .field("db_url_routing", &self.db_url_routing)
            .finish_non_exhaustive()
    }
}

impl RegisteredCommand {
    pub const fn descriptor(&self) -> CommandDescriptor {
        self.descriptor
    }

    pub const fn db_url_routing(&self) -> DbUrlRouting {
        self.db_url_routing
    }

    pub(super) fn granted(mut self, grants: &[KernelGrant]) -> Self {
        self.descriptor = grants.iter().fold(self.descriptor, |descriptor, grant| {
            descriptor.with_grant(*grant)
        });
        self
    }

    pub fn run<'a>(&'a self, ctx: &'a CommandContext) -> CommandFuture<'a> {
        (self.run)(&self.matches, ctx)
    }
}

pub(super) fn group_command<T: CliCommandGroup>() -> Command {
    T::augment_subcommands(
        Command::new(T::NAME)
            .about(T::ABOUT)
            .display_order(T::DISPLAY_ORDER)
            .subcommand_required(true)
            .arg_required_else_help(true),
    )
}

pub(super) fn resolve_group<T: CliCommandGroup>(
    matches: &ArgMatches,
) -> Result<RegisteredCommand, clap::Error> {
    let parsed = T::from_arg_matches(matches)?;
    Ok(RegisteredCommand {
        descriptor: parsed.descriptor(),
        db_url_routing: parsed.db_url_routing(),
        matches: matches.clone(),
        run: run_group::<T>,
    })
}

fn run_group<'a, T: CliCommandGroup>(
    matches: &'a ArgMatches,
    ctx: &'a CommandContext,
) -> CommandFuture<'a> {
    Box::pin(async move {
        let parsed = T::from_arg_matches(matches)?;
        parsed.run(ctx).await
    })
}

pub(super) fn leaf_command<T: CliCommandLeaf>() -> Command {
    T::augment_args(
        Command::new(T::NAME)
            .about(T::ABOUT)
            .display_order(T::DISPLAY_ORDER),
    )
}

pub(super) fn resolve_leaf<T: CliCommandLeaf>(
    matches: &ArgMatches,
) -> Result<RegisteredCommand, clap::Error> {
    let parsed = T::from_arg_matches(matches)?;
    Ok(RegisteredCommand {
        descriptor: parsed.descriptor(),
        db_url_routing: parsed.db_url_routing(),
        matches: matches.clone(),
        run: run_leaf::<T>,
    })
}

fn run_leaf<'a, T: CliCommandLeaf>(
    matches: &'a ArgMatches,
    ctx: &'a CommandContext,
) -> CommandFuture<'a> {
    Box::pin(async move {
        let parsed = T::from_arg_matches(matches)?;
        parsed.run(ctx).await
    })
}
