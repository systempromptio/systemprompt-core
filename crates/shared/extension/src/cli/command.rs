//! Command declarations, the traits a command implements, and the host it
//! runs against.
//!
//! A member declares a clap [`CliCommandGroup`] or [`CliCommandLeaf`] with
//! `Contributions::command(CommandDecl::…)`, naming the [`CommandParent`] it
//! attaches under and the [`KernelGrant`]s it requests. The declaration is
//! type-erased: it carries the clap command builder and a resolver that
//! parses matches into a [`ParsedCommand`], whose future runs the command
//! against a [`CommandHost`] and returns its [`CommandOutput`]. The CLI kernel
//! renders the output; a command never prints.
//!
//! `DISPLAY_ORDER` places a top-level group in `--help`. The kernel's own
//! groups hold the slots `10` (`core`), `20` (`infra`), `30` (`admin`), `60`
//! (`web`) and `70` (`plugins`); `40` and `50` are the `cloud` and `analytics`
//! slots. A declaration that names no slot gets [`REGISTERED_DISPLAY_ORDER`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use clap::{ArgMatches, Args, Command, Subcommand};
use systemprompt_manifest::Profile;
use systemprompt_traits::DatabaseHandle;

use super::KernelGrant;
use super::child::ChildSupervisor;
use super::descriptor::{CommandDescriptor, DbUrlRouting, DescribeCommand};
use super::output::CommandOutput;
use super::prompt::Prompter;
use super::settings::CliConfig;
use crate::contribute::Resolved;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type CommandFuture<'a> = BoxFuture<'a, Result<CommandOutput, CommandError>>;

pub const REGISTERED_DISPLAY_ORDER: usize = 1000;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct CommandError(#[from] Box<dyn std::error::Error + Send + Sync>);

impl CommandError {
    pub fn new(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self(Box::new(error))
    }
}

/// A kernel group a contributed command attaches under.
pub trait CommandParent: 'static {
    const PATH: &'static [&'static str];
}

pub mod parents {
    use super::CommandParent;

    #[derive(Debug, Clone, Copy)]
    pub struct Core;
    #[derive(Debug, Clone, Copy)]
    pub struct Infra;
    #[derive(Debug, Clone, Copy)]
    pub struct Admin;
    #[derive(Debug, Clone, Copy)]
    pub struct Web;
    #[derive(Debug, Clone, Copy)]
    pub struct Plugins;
    #[derive(Debug, Clone, Copy)]
    pub struct ContentGroup;

    impl CommandParent for Core {
        const PATH: &'static [&'static str] = &["core"];
    }
    impl CommandParent for Infra {
        const PATH: &'static [&'static str] = &["infra"];
    }
    impl CommandParent for Admin {
        const PATH: &'static [&'static str] = &["admin"];
    }
    impl CommandParent for Web {
        const PATH: &'static [&'static str] = &["web"];
    }
    impl CommandParent for Plugins {
        const PATH: &'static [&'static str] = &["plugins"];
    }
    impl CommandParent for ContentGroup {
        const PATH: &'static [&'static str] = &["core", "content"];
    }
}

/// A clap subcommand group a member contributes.
pub trait CliCommandGroup: Subcommand + DescribeCommand + Send + Sized + 'static {
    const NAME: &'static str;
    const ABOUT: &'static str = "";
    const DISPLAY_ORDER: usize = REGISTERED_DISPLAY_ORDER;

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Unsupported
    }

    fn run(
        self,
        host: &dyn CommandHost,
    ) -> impl Future<Output = Result<CommandOutput, CommandError>> + Send;
}

/// A single clap command with its own arguments a member contributes.
pub trait CliCommandLeaf: Args + DescribeCommand + Send + Sized + 'static {
    const NAME: &'static str;
    const ABOUT: &'static str = "";
    const DISPLAY_ORDER: usize = REGISTERED_DISPLAY_ORDER;

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Unsupported
    }

    fn run(
        self,
        host: &dyn CommandHost,
    ) -> impl Future<Output = Result<CommandOutput, CommandError>> + Send;
}

/// What a command may ask of the CLI kernel.
pub trait CommandHost: Send + Sync {
    fn output(&self) -> &CliConfig;
    fn prompter(&self) -> &dyn Prompter;
    fn is_database_scoped(&self) -> bool;
    fn profile(&self) -> Result<&Profile, CommandError>;
    fn db(&self) -> BoxFuture<'_, Result<Arc<dyn DatabaseHandle>, CommandError>>;
    fn resolved(&self) -> BoxFuture<'_, Result<Resolved<'_>, CommandError>>;
    fn children(&self) -> &dyn ChildSupervisor;
}

/// The kind of clap node a declaration contributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Group,
    Leaf,
}

type BuildFn = fn() -> Command;
type ResolveFn = fn(&ArgMatches) -> Result<ParsedCommand, clap::Error>;
type RunFn = for<'a> fn(&'a ArgMatches, &'a dyn CommandHost) -> CommandFuture<'a>;

/// One contributed command: where it attaches, the grants it requests, and
/// its type-erased clap builder and resolver.
#[derive(Clone, Copy)]
pub struct CommandDecl {
    parent: &'static [&'static str],
    name: &'static str,
    kind: CommandKind,
    grants: &'static [KernelGrant],
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

    /// Adds the grants the kernel admitted for this command's path.
    #[must_use]
    pub fn granted(mut self, grants: &[KernelGrant]) -> Self {
        self.descriptor = grants.iter().fold(self.descriptor, |descriptor, grant| {
            descriptor.with_grant(*grant)
        });
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
        db_url_routing: parsed.db_url_routing(),
        matches: matches.clone(),
        run: run_group::<T>,
    })
}

fn run_group<'a, T: CliCommandGroup>(
    matches: &'a ArgMatches,
    host: &'a dyn CommandHost,
) -> CommandFuture<'a> {
    Box::pin(async move {
        let parsed = T::from_arg_matches(matches)
            .map_err(CommandError::new)?;
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
        db_url_routing: parsed.db_url_routing(),
        matches: matches.clone(),
        run: run_leaf::<T>,
    })
}

fn run_leaf<'a, T: CliCommandLeaf>(
    matches: &'a ArgMatches,
    host: &'a dyn CommandHost,
) -> CommandFuture<'a> {
    Box::pin(async move {
        let parsed = T::from_arg_matches(matches)
            .map_err(CommandError::new)?;
        parsed.run(host).await
    })
}
