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
//! kernel group is attached in place, so the parent keeps its position.
//!
//! Precedence is fixed and never silent:
//!
//! - A kernel command owns its path; a registration on it is refused
//!   ([`RegistryError::ShadowsKernel`]).
//! - Two registrations of one path are refused ([`RegistryError::Duplicate`]).
//! - A registration may attach under a registered group (`core content
//!   popular` under `core content`). It is listed in the group's `--help`
//!   after the group's own subcommands, parses its own arguments, and
//!   resolves to its own descriptor: [`CommandRegistry::resolve`] picks the
//!   deepest registration on the parsed path, so neither the parent's
//!   descriptor nor its grants apply to it, and the child's never apply to the
//!   parent's own subcommands. A name that collides with one of the group's
//!   own subcommands is refused ([`RegistryError::ShadowsGroup`]).
//! - A registration under a parent that does not exist is refused
//!   ([`RegistryError::UnknownParent`]).
//!
//! A [`KernelGrant`] relaxes a requirement the runner imposes, so the kernel
//! holds the closed list of registration paths allowed each one:
//! `cloud` → cloud-operator, `analytics` → local-export and skip-validation,
//! `admin mcp` → child-host. Each listed path is reserved for one owning
//! type (its `std::any::type_name`); a registration there by any other type
//! is refused whether or not it declares a grant
//! ([`RegistryError::ReservedPath`]). A registration declaring a grant the
//! list does not give its path and type is refused
//! ([`RegistryError::GrantNotListed`]), and a
//! parsed descriptor holding a grant its registration did not declare is
//! refused at resolution ([`RegistryError::UndeclaredGrant`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::{ArgMatches, Command};

use super::command::{
    CliCommandGroup, CliCommandLeaf, RegisteredCommand, group_command, leaf_command, resolve_group,
    resolve_leaf,
};
use super::grants::{KernelGrant, listed_for, reserved_at};
use super::registry_error::{RegistryError, ResolveError};

/// One `inventory` submission: a type-erased [`CliCommandGroup`].
#[derive(Clone, Copy)]
pub struct CliCommandRegistration {
    parent: &'static [&'static str],
    name: &'static str,
    group: fn() -> &'static str,
    grants: &'static [KernelGrant],
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
            grants: T::GRANTS,
            command: group_command::<T>,
            resolve: resolve_group::<T>,
        }
    }

    pub const fn leaf<T: CliCommandLeaf>() -> Self {
        Self {
            parent: T::PARENT,
            name: T::NAME,
            group: std::any::type_name::<T>,
            grants: T::GRANTS,
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

    pub const fn grants(&self) -> &'static [KernelGrant] {
        self.grants
    }

    const fn key(&self) -> (&'static [&'static str], &'static str) {
        (self.parent, self.name)
    }

    fn is_at(&self, path: &[&str]) -> bool {
        path.split_last()
            .is_some_and(|(name, parent)| *name == self.name && parent == self.parent)
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
        if let Some((entry, owner)) = entries.iter().find_map(|entry| {
            reserved_at(entry.parent, entry.name)
                .filter(|reserved| reserved.owner() != entry.group())
                .map(|reserved| (entry, reserved.owner()))
        }) {
            return Err(RegistryError::ReservedPath {
                path: entry.path(),
                group: entry.group(),
                owner,
            });
        }
        if let Some((entry, grant)) = entries.iter().find_map(|entry| {
            entry
                .grants
                .iter()
                .find(|grant| !listed_for(entry.parent, entry.name, entry.group(), **grant))
                .map(|grant| (entry, *grant))
        }) {
            return Err(RegistryError::GrantNotListed {
                path: entry.path(),
                group: entry.group(),
                grant,
            });
        }
        Ok(Self { entries })
    }

    pub fn registrations(&self) -> &[CliCommandRegistration] {
        &self.entries
    }

    pub fn assemble(&self, mut root: Command) -> Result<Command, RegistryError> {
        for entry in &self.entries {
            let parent = self.entries.iter().find(|other| other.is_at(entry.parent));
            root = attach(root, entry.parent, (entry.command)(), entry, parent)?;
        }
        Ok(root)
    }

    pub fn resolve(&self, matches: &ArgMatches) -> Result<Option<RegisteredCommand>, ResolveError> {
        let mut path: Vec<&str> = Vec::new();
        let mut current = matches;
        let mut deepest = None;
        while let Some((name, sub)) = current.subcommand() {
            path.push(name);
            if let Some(entry) = self.entries.iter().find(|entry| entry.is_at(&path)) {
                deepest = Some((entry, sub));
            }
            current = sub;
        }
        let Some((entry, sub)) = deepest else {
            return Ok(None);
        };
        let registered = (entry.resolve)(sub)?;
        let descriptor = registered.descriptor();
        if let Some(grant) = KernelGrant::ALL
            .into_iter()
            .find(|grant| descriptor.holds(*grant) && !entry.grants.contains(grant))
        {
            return Err(RegistryError::UndeclaredGrant {
                path: entry.path(),
                group: entry.group(),
                grant,
            }
            .into());
        }
        Ok(Some(registered.granted(entry.grants)))
    }
}

fn attach(
    mut parent: Command,
    rest: &[&str],
    child: Command,
    entry: &CliCommandRegistration,
    registered_parent: Option<&CliCommandRegistration>,
) -> Result<Command, RegistryError> {
    let Some((head, tail)) = rest.split_first() else {
        if parent.find_subcommand(child.get_name()).is_some() {
            return Err(registered_parent.map_or_else(
                || RegistryError::ShadowsKernel {
                    path: entry.path(),
                    group: entry.group(),
                },
                |owner| RegistryError::ShadowsGroup {
                    path: entry.path(),
                    group: entry.group(),
                    parent: owner.group(),
                },
            ));
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
    *existing = attach(std::mem::take(existing), tail, child, entry, registered_parent)?;
    Ok(parent)
}
