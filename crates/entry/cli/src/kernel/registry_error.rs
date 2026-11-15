//! Why the command registry refused an entry, or a command line could not be
//! resolved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::cli::KernelGrant;
use systemprompt_extension::reserved::CoreId;

use super::registry::CommandOwner;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("CLI command `{path}` is contributed by both `{first}` and `{second}`")]
    Duplicate {
        path: String,
        first: CommandOwner,
        second: CommandOwner,
    },
    #[error("CLI command `{path}` contributed by `{claimant}` shadows a kernel command")]
    ShadowsKernel { path: String, claimant: CommandOwner },
    #[error("CLI command `{path}` contributed by `{claimant}` names a parent that does not exist")]
    UnknownParent { path: String, claimant: CommandOwner },
    #[error(
        "CLI command `{path}` contributed by `{claimant}` shadows a subcommand of the group \
         contributed by `{parent}`"
    )]
    ShadowsGroup {
        path: String,
        claimant: CommandOwner,
        parent: CommandOwner,
    },
    #[error("CLI command `{path}` is reserved for `{owner:?}` and cannot be claimed by `{claimant}`")]
    ReservedPath {
        path: String,
        claimant: CommandOwner,
        owner: CoreId,
    },
    #[error(
        "CLI command `{path}` contributed by `{claimant}` requests `{grant}`, which the kernel \
         does not grant to that path"
    )]
    GrantNotListed {
        path: String,
        claimant: CommandOwner,
        grant: KernelGrant,
    },
    #[error(
        "CLI command `{path}` contributed by `{owner}` resolved a descriptor holding `{grant}` \
         without requesting it"
    )]
    UndeclaredGrant {
        path: String,
        owner: CommandOwner,
        grant: KernelGrant,
    },
}

/// Why a parsed command line could not be resolved to a registry entry.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error(transparent)]
    Parse(#[from] clap::Error),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}
