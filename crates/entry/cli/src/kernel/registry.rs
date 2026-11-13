//! Compile-time registration of CLI command groups.
//!
//! A command group is a clap `Subcommand` enum that implements
//! [`CliCommandGroup`] and is submitted with
//! [`register_cli_command!`](crate::register_cli_command); a single command
//! is a clap `Args` struct that implements [`CliCommandLeaf`] and is
//! submitted with `register_cli_command!(leaf T)`. The kernel collects
//! every submission through `inventory`, attaches each group to the command
//! tree at the place it declares (top level, or under an existing kernel
//! group such as `admin`), and dispatches a parsed invocation to the group's
//! `run` with the shared [`CommandContext`](crate::kernel::CommandContext).
//!
//! The group declares its bootstrap requirements through
//! [`DescribeCommand`](crate::kernel::DescribeCommand), the same descriptor
//! every kernel command carries, so the runner resolves profile, secrets,
//! paths, database, routing and data-impact gates for a registered group
//! exactly as for a built-in one. The traits and the parsed form live in
//! [`super::command`].
//!
//! Registrations are attached in `(parent, name)` order, so `--help` lists
//! them deterministically after the kernel groups; a group nested under a
//! kernel group is attached in place, so the parent keeps its position. Two
//! registrations of one path, a registration that shadows a kernel command,
//! and a registration under a parent that does not exist are refused with a
//! [`RegistryError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::{ArgMatches, Command};

use super::command::{
    CliCommandGroup, CliCommandLeaf, RegisteredCommand, group_command, leaf_command, resolve_group,
    resolve_leaf,
};

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
            .finish_non_exhaustive()
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

    pub const fn leaf<T: CliCommandLeaf>() -> Self {
        Self {
            parent: T::PARENT,
            name: T::NAME,
            group: std::any::type_name::<T>,
            command: leaf_command::<T>,
            resolve: resolve_leaf::<T>,
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

    const fn key(&self) -> (&'static [&'static str], &'static str) {
        (self.parent, self.name)
    }
}

#[macro_export]
macro_rules! register_cli_command {
    (leaf $leaf:ty) => {
        $crate::__inventory::submit! {
            $crate::kernel::CliCommandRegistration::leaf::<$leaf>()
        }
    };
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
    mut parent: Command,
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
        .get_subcommands_mut()
        .find(|sub| sub.get_name() == *head)
    else {
        return Err(RegistryError::UnknownParent {
            path: entry.path(),
            group: entry.group(),
        });
    };
    *existing = attach(std::mem::take(existing), tail, child, entry)?;
    Ok(parent)
}
