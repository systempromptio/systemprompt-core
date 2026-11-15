//! The validated command registry the CLI builds its tree from.
//!
//! The registry is built from the composition's [`DeclaredCommands`] and the
//! kernel's own contributed built-ins, before the profile is loaded. Every
//! entry that claims a [`RESERVED_CLI_PATHS`] path, or requests a
//! [`KernelGrant`], is re-checked here whatever handed it over: the path's
//! listed [`CoreId`] must own it, every requested grant must be listed for
//! that path, and no owner may hold reserved paths under two core ids. A
//! command attaches under a kernel group or under another entry, and never
//! shadows an existing subcommand.
//!
//! Resolution returns the parsed command with its owner, so the runner can
//! refuse a command whose owner the profile disables before it runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use clap::{ArgMatches, Command};
use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::cli::{CommandDecl, KernelGrant, ParsedCommand};
use systemprompt_extension::reserved::{CoreId, RESERVED_CLI_PATHS};
use systemprompt_identifiers::ExtensionId;

use super::registry_error::{RegistryError, ResolveError};

/// Who contributed a registry entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CommandOwner {
    Kernel,
    Member(ExtensionId),
}

impl CommandOwner {
    fn holds(&self, core: CoreId) -> bool {
        match self {
            Self::Kernel => core == CoreId::Kernel,
            Self::Member(id) => core != CoreId::Kernel && id.as_str() == core.id(),
        }
    }
}

impl std::fmt::Display for CommandOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kernel => f.write_str(CoreId::Kernel.id()),
            Self::Member(id) => f.write_str(id.as_str()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegistryEntry {
    owner: CommandOwner,
    decl: CommandDecl,
}

impl RegistryEntry {
    #[must_use]
    pub const fn owner(&self) -> &CommandOwner {
        &self.owner
    }

    #[must_use]
    pub const fn decl(&self) -> &CommandDecl {
        &self.decl
    }

    fn path_text(&self) -> String {
        self.decl.path().join(" ")
    }

    fn is_at(&self, path: &[&str]) -> bool {
        path.split_last().is_some_and(|(name, parent)| {
            *name == self.decl.name() && parent == self.decl.parent()
        })
    }
}

/// The kernel's own contributed built-ins: the `build` group and the
/// child-host `admin mcp` group.
#[must_use]
pub fn kernel_commands() -> Vec<CommandDecl> {
    use systemprompt_extension::cli::parents;
    vec![
        CommandDecl::group::<crate::commands::build::BuildCommands>(),
        CommandDecl::group_under::<parents::Admin, crate::commands::admin::mcp::McpHostCommands>()
            .requesting(&[KernelGrant::ChildHost]),
    ]
}

/// A command line resolved to a registry entry: its owner, its path and the
/// parsed command.
#[derive(Debug, Clone)]
pub struct ResolvedCommand {
    pub owner: CommandOwner,
    pub path: Vec<&'static str>,
    pub parsed: ParsedCommand,
}

/// The validated set of contributed commands.
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    entries: Vec<RegistryEntry>,
}

impl CommandRegistry {
    pub fn build(declared: &DeclaredCommands, kernel: &[CommandDecl]) -> Result<Self, RegistryError> {
        let mut entries: Vec<RegistryEntry> = kernel
            .iter()
            .map(|decl| RegistryEntry {
                owner: CommandOwner::Kernel,
                decl: *decl,
            })
            .collect();
        entries.extend(declared.commands().iter().map(|command| RegistryEntry {
            owner: CommandOwner::Member(command.owner.clone()),
            decl: command.decl,
        }));
        refuse_duplicates(&entries)?;
        recheck_reserved(&entries)?;
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[RegistryEntry] {
        &self.entries
    }

    pub fn assemble(&self, mut root: Command) -> Result<Command, RegistryError> {
        for entry in &self.entries {
            let parent = self
                .entries
                .iter()
                .find(|other| other.is_at(entry.decl.parent()));
            root = attach(root, entry.decl.parent(), entry.decl.command(), entry, parent)?;
        }
        Ok(root)
    }

    pub fn resolve(
        &self,
        matches: &ArgMatches,
    ) -> Result<Option<ResolvedCommand>, ResolveError> {
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
        let parsed = entry.decl.resolve(sub)?;
        let descriptor = parsed.descriptor();
        if let Some(grant) = KernelGrant::ALL
            .into_iter()
            .find(|grant| descriptor.holds(*grant) && !entry.decl.grants().contains(grant))
        {
            return Err(RegistryError::UndeclaredGrant {
                path: entry.path_text(),
                owner: entry.owner.clone(),
                grant,
            }
            .into());
        }
        Ok(Some(ResolvedCommand {
            owner: entry.owner.clone(),
            path: entry.decl.path(),
            parsed: parsed.granted(entry.decl.grants()),
        }))
    }
}

fn refuse_duplicates(entries: &[RegistryEntry]) -> Result<(), RegistryError> {
    for (index, entry) in entries.iter().enumerate() {
        if let Some(other) = entries[index + 1..]
            .iter()
            .find(|other| other.decl.path() == entry.decl.path())
        {
            return Err(RegistryError::Duplicate {
                path: entry.path_text(),
                first: entry.owner.clone(),
                second: other.owner.clone(),
            });
        }
    }
    Ok(())
}

fn recheck_reserved(entries: &[RegistryEntry]) -> Result<(), RegistryError> {
    let mut holders: HashMap<CommandOwner, CoreId> = HashMap::new();
    for entry in entries {
        let path = entry.decl.path();
        let reserved = RESERVED_CLI_PATHS
            .iter()
            .find(|reserved| reserved.path == path.as_slice());
        if let Some(reserved) = reserved {
            if !entry.owner.holds(reserved.owner) {
                return Err(RegistryError::ReservedPath {
                    path: entry.path_text(),
                    claimant: entry.owner.clone(),
                    owner: reserved.owner,
                });
            }
            if let Some(previous) = holders.insert(entry.owner.clone(), reserved.owner)
                && previous != reserved.owner
            {
                return Err(RegistryError::PrivilegeOwnerMismatch {
                    path: entry.path_text(),
                    claimant: entry.owner.clone(),
                    expected: reserved.owner,
                });
            }
        }
        if let Some(grant) = entry
            .decl
            .grants()
            .iter()
            .find(|grant| !reserved.is_some_and(|reserved| reserved.grants.contains(grant)))
        {
            return Err(RegistryError::GrantNotListed {
                path: entry.path_text(),
                claimant: entry.owner.clone(),
                grant: *grant,
            });
        }
    }
    Ok(())
}

fn attach(
    mut parent: Command,
    rest: &[&str],
    child: Command,
    entry: &RegistryEntry,
    registered_parent: Option<&RegistryEntry>,
) -> Result<Command, RegistryError> {
    let Some((head, tail)) = rest.split_first() else {
        if parent.find_subcommand(child.get_name()).is_some() {
            return Err(registered_parent.map_or_else(
                || RegistryError::ShadowsKernel {
                    path: entry.path_text(),
                    claimant: entry.owner.clone(),
                },
                |owner| RegistryError::ShadowsGroup {
                    path: entry.path_text(),
                    claimant: entry.owner.clone(),
                    parent: owner.owner.clone(),
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
            path: entry.path_text(),
            claimant: entry.owner.clone(),
        });
    };
    *existing = attach(
        std::mem::take(existing),
        tail,
        child,
        entry,
        registered_parent,
    )?;
    Ok(parent)
}
