//! Compile-time registration of CLI command groups.
//!
//! A command group is a clap `Subcommand` enum that implements
//! [`CliCommandGroup`] and is submitted with
//! [`register_cli_command!`](crate::register_cli_command). The kernel collects
//! every submission through `inventory`, attaches each group to the command
//! tree at the place it declares (top level, or under an existing kernel
//! group such as `admin`), and dispatches a parsed invocation to the group's
//! `run` with the shared [`CommandContext`].
//!
//! The group declares its bootstrap requirements through
//! [`DescribeCommand`], the same descriptor every kernel command carries, so
//! the runner resolves profile, secrets, paths, database, routing and
//! data-impact gates for a registered group exactly as for a built-in one.
//!
//! Registrations are attached in `(parent, name)` order, so `--help` lists
//! them deterministically after the kernel groups. Two registrations of one
//! path, a registration that shadows a kernel command, and a registration
//! under a parent that does not exist are refused with a [`RegistryError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;

use clap::{ArgMatches, Command, FromArgMatches, Subcommand};

use crate::context::CommandContext;
use crate::descriptor::{CommandDescriptor, DbUrlRouting, DescribeCommand};

pub type CommandFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>>;

type RunFn = for<'a> fn(&'a ArgMatches, &'a CommandContext) -> CommandFuture<'a>;

/// A command group contributed to the CLI from outside the kernel's own tree.
///
/// `PARENT` is the path of the kernel group the group attaches under (empty
/// for a top-level domain), `NAME` its subcommand name and `ABOUT` its
/// one-line help. [`DescribeCommand::descriptor`] returns the complete
/// descriptor for the parsed command, data impact included.
pub trait CliCommandGroup: Subcommand + DescribeCommand + Sized + 'static {
    const PARENT: &'static [&'static str];
    const NAME: &'static str;
    const ABOUT: &'static str;

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Unsupported
    }

    fn run(self, ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>>;
}

/// One `inventory` submission: a type-erased [`CliCommandGroup`].
#[derive(Clone, Copy)]
pub struct CliCommandRegistration {
    parent: &'static [&'static str],
    name: &'static str,
    group: fn() -> &'static str,
    command: fn() -> Command,
    resolve: fn(&ArgMatches) -> Result<RegisteredCommand, clap::Error>,
}

inventory::collect!(CliCommandRegistration);

impl std::fmt::Debug for CliCommandRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliCommandRegistration")
            .field("path", &self.path())
            .field("group", &(self.group)())
            .finish()
    }
}

impl CliCommandRegistration {
    pub const fn of<T: CliCommandGroup>() -> Self {
        Self {
            parent: T::PARENT,
            name: T::NAME,
            group: std::any::type_name::<T>,
            command: group_command::<T>,
            resolve: resolve_group::<T>,
        }
    }

    pub fn path(&self) -> String {
        self.parent
            .iter()
            .chain(std::iter::once(&self.name))
            .copied()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn group(&self) -> &'static str {
        (self.group)()
    }

    fn key(&self) -> (&'static [&'static str], &'static str) {
        (self.parent, self.name)
    }
}

#[macro_export]
macro_rules! register_cli_command {
    ($group:ty) => {
        $crate::__inventory::submit! {
            $crate::kernel::CliCommandRegistration::of::<$group>()
        }
    };
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("CLI command `{path}` is registered by both `{first}` and `{second}`")]
    Duplicate {
        path: String,
        first: &'static str,
        second: &'static str,
    },
    #[error("CLI command `{path}` registered by `{group}` shadows a kernel command")]
    ShadowsKernel { path: String, group: &'static str },
    #[error("CLI command `{path}` registered by `{group}` names a parent that does not exist")]
    UnknownParent { path: String, group: &'static str },
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

    pub fn run<'a>(&'a self, ctx: &'a CommandContext) -> CommandFuture<'a> {
        (self.run)(&self.matches, ctx)
    }
}

/// The validated set of registered command groups.
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    entries: Vec<CliCommandRegistration>,
}

impl CommandRegistry {
    pub fn collected() -> Result<Self, RegistryError> {
        Self::from_registrations(
            inventory::iter::<CliCommandRegistration>
                .into_iter()
                .copied(),
        )
    }

    pub fn from_registrations(
        registrations: impl IntoIterator<Item = CliCommandRegistration>,
    ) -> Result<Self, RegistryError> {
        let mut entries: Vec<CliCommandRegistration> = registrations.into_iter().collect();
        entries.sort_by_key(CliCommandRegistration::key);
        if let Some(pair) = entries
            .windows(2)
            .find(|pair| pair[0].key() == pair[1].key())
        {
            let (first, second) = (pair[0].group(), pair[1].group());
            let (first, second) = if first <= second {
                (first, second)
            } else {
                (second, first)
            };
            return Err(RegistryError::Duplicate {
                path: pair[0].path(),
                first,
                second,
            });
        }
        Ok(Self { entries })
    }

    pub fn registrations(&self) -> &[CliCommandRegistration] {
        &self.entries
    }

    pub fn assemble(&self, mut root: Command) -> Result<Command, RegistryError> {
        for entry in &self.entries {
            root = attach(root, entry.parent, (entry.command)(), entry)?;
        }
        Ok(root)
    }

    pub fn resolve(&self, matches: &ArgMatches) -> Result<Option<RegisteredCommand>, clap::Error> {
        let mut path: Vec<&str> = Vec::new();
        let mut current = matches;
        while let Some((name, sub)) = current.subcommand() {
            if let Some(entry) = self
                .entries
                .iter()
                .find(|entry| entry.name == name && entry.parent == path.as_slice())
            {
                return (entry.resolve)(sub).map(Some);
            }
            path.push(name);
            current = sub;
        }
        Ok(None)
    }
}

fn attach(
    parent: Command,
    rest: &[&str],
    child: Command,
    entry: &CliCommandRegistration,
) -> Result<Command, RegistryError> {
    let Some((head, tail)) = rest.split_first() else {
        if parent.find_subcommand(child.get_name()).is_some() {
            return Err(RegistryError::ShadowsKernel {
                path: entry.path(),
                group: entry.group(),
            });
        }
        return Ok(parent.subcommand(child));
    };
    let Some(existing) = parent
        .get_subcommands()
        .find(|sub| sub.get_name() == *head)
        .cloned()
    else {
        return Err(RegistryError::UnknownParent {
            path: entry.path(),
            group: entry.group(),
        });
    };
    let replaced = attach(existing, tail, child, entry)?;
    Ok(parent.mut_subcommand(head, |_| replaced))
}

fn group_command<T: CliCommandGroup>() -> Command {
    T::augment_subcommands(
        Command::new(T::NAME)
            .about(T::ABOUT)
            .subcommand_required(true)
            .arg_required_else_help(true),
    )
}

fn resolve_group<T: CliCommandGroup>(
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
