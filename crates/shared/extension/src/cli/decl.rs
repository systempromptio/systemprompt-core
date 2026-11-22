//! The type-erased command declaration and its parsed invocation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::{ArgMatches, Command};

use super::{CommandClaim, KernelGrant};
use super::command::{
    CliCommandGroup, CliCommandLeaf, CommandError, CommandFuture, CommandHost, CommandParent,
};
use super::descriptor::CommandDescriptor;
use super::impact::DbUrlRouting;

/// The kind of clap node a declaration contributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Group,
    Leaf,
}

type BuildFn = fn() -> Command;
type ResolveFn = fn(&ArgMatches) -> Result<ParsedCommand, clap::Error>;
type RunFn = for<'a> fn(&'a ArgMatches, &'a dyn CommandHost) -> CommandFuture<'a>;

/// One contributed command: where it attaches, the grants it requests, the
/// claims its commands make, and its type-erased clap builder and resolver.
#[derive(Clone, Copy)]
pub struct CommandDecl {
    parent: &'static [&'static str],
    name: &'static str,
    kind: CommandKind,
    grants: &'static [KernelGrant],
    claims: &'static [CommandClaim],
    command: BuildFn,
    resolve: ResolveFn,
}

impl CommandDecl {
    pub fn group<T: CliCommandGroup>() -> Self {
        Self::group_at::<T>(&[])
    }

    pub fn group_under<P: CommandParent, T: CliCommandGroup>() -> Self {
        Self::group_at::<T>(P::PATH)
    }

    pub fn leaf_under<P: CommandParent, T: CliCommandLeaf>() -> Self {
        Self {
            parent: P::PATH,
            name: T::NAME,
            kind: CommandKind::Leaf,
            grants: &[],
            claims: &[],
            command: leaf_command::<T>,
            resolve: resolve_leaf::<T>,
        }
    }

    fn group_at<T: CliCommandGroup>(parent: &'static [&'static str]) -> Self {
        Self {
            parent,
            name: T::NAME,
            kind: CommandKind::Group,
            grants: &[],
            claims: &[],
            command: group_command::<T>,
            resolve: resolve_group::<T>,
        }
    }

    #[must_use]
    pub const fn requesting(mut self, g: &'static [KernelGrant]) -> Self {
        self.grants = g;
        self
    }

    #[must_use]
    pub const fn claiming(mut self, c: &'static [CommandClaim]) -> Self {
        self.claims = c;
        self
    }

    #[must_use]
    pub const fn parent(&self) -> &'static [&'static str] {
        self.parent
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub fn path(&self) -> Vec<&'static str> {
        let mut path = self.parent.to_vec();
        path.push(self.name);
        path
    }

    #[must_use]
    pub const fn kind(&self) -> CommandKind {
        self.kind
    }

    #[must_use]
    pub const fn grants(&self) -> &'static [KernelGrant] {
        self.grants
    }

    #[must_use]
    pub const fn claims(&self) -> &'static [CommandClaim] {
        self.claims
    }

    #[must_use]
    pub fn command(&self) -> Command {
        (self.command)()
    }

    pub fn resolve(&self, matches: &ArgMatches) -> Result<ParsedCommand, clap::Error> {
        (self.resolve)(matches)
    }
}

impl std::fmt::Debug for CommandDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandDecl")
            .field("parent", &self.parent)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("grants", &self.grants)
            .field("claims", &self.claims)
            .finish_non_exhaustive()
    }
}

/// A parsed invocation of a contributed command, ready to run.
#[derive(Clone)]
pub struct ParsedCommand {
    descriptor: CommandDescriptor,
    db_url_routing: DbUrlRouting,
    matches: ArgMatches,
    run: RunFn,
}

impl std::fmt::Debug for ParsedCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedCommand")
            .field("descriptor", &self.descriptor)
            .field("db_url_routing", &self.db_url_routing)
            .finish_non_exhaustive()
    }
}

impl ParsedCommand {
    #[must_use]
    pub const fn descriptor(&self) -> CommandDescriptor {
        self.descriptor
    }

    #[must_use]
    pub const fn db_url_routing(&self) -> DbUrlRouting {
        self.db_url_routing
    }

    #[must_use]
    pub fn granted(mut self, grants: &[KernelGrant], claims: &[CommandClaim]) -> Self {
        self.descriptor = grants.iter().fold(self.descriptor, |descriptor, grant| {
            descriptor.with_grant(*grant)
        });
        self.db_url_routing = if claims.contains(&CommandClaim::DirectDatabase) {
            DbUrlRouting::Direct
        } else {
            DbUrlRouting::Unsupported
        };
        self
    }

    pub fn run<'a>(&'a self, host: &'a dyn CommandHost) -> CommandFuture<'a> {
        (self.run)(&self.matches, host)
    }
}

fn group_command<T: CliCommandGroup>() -> Command {
    T::augment_subcommands(
        Command::new(T::NAME)
            .about(T::ABOUT)
            .display_order(T::DISPLAY_ORDER)
            .subcommand_required(true)
            .arg_required_else_help(true),
    )
}

fn resolve_group<T: CliCommandGroup>(matches: &ArgMatches) -> Result<ParsedCommand, clap::Error> {
    let parsed = T::from_arg_matches(matches)?;
    Ok(ParsedCommand {
        descriptor: parsed.descriptor(),
        db_url_routing: DbUrlRouting::Unsupported,
        matches: matches.clone(),
        run: run_group::<T>,
    })
}

fn run_group<'a, T: CliCommandGroup>(
    matches: &'a ArgMatches,
    host: &'a dyn CommandHost,
) -> CommandFuture<'a> {
    Box::pin(async move {
        let parsed = T::from_arg_matches(matches).map_err(CommandError::new)?;
        parsed.run(host).await
    })
}

fn leaf_command<T: CliCommandLeaf>() -> Command {
    T::augment_args(
        Command::new(T::NAME)
            .about(T::ABOUT)
            .display_order(T::DISPLAY_ORDER),
    )
}

fn resolve_leaf<T: CliCommandLeaf>(matches: &ArgMatches) -> Result<ParsedCommand, clap::Error> {
    let parsed = T::from_arg_matches(matches)?;
    Ok(ParsedCommand {
        descriptor: parsed.descriptor(),
        db_url_routing: DbUrlRouting::Unsupported,
        matches: matches.clone(),
        run: run_leaf::<T>,
    })
}

fn run_leaf<'a, T: CliCommandLeaf>(
    matches: &'a ArgMatches,
    host: &'a dyn CommandHost,
) -> CommandFuture<'a> {
    Box::pin(async move {
        let parsed = T::from_arg_matches(matches).map_err(CommandError::new)?;
        parsed.run(host).await
    })
}
