//! The typed refusals of command registration and resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::KernelGrant;

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
    #[error(
        "CLI command `{path}` registered by `{group}` shadows a subcommand of the group \
         registered by `{parent}`"
    )]
    ShadowsGroup {
        path: String,
        group: &'static str,
        parent: &'static str,
    },
    #[error("CLI command `{path}` is reserved for `{owner}` and cannot be registered by `{group}`")]
    ReservedPath {
        path: String,
        group: &'static str,
        owner: &'static str,
    },
    #[error(
        "CLI command `{path}` registered by `{group}` declares `{grant}`, which the kernel does \
         not grant to that path"
    )]
    GrantNotListed {
        path: String,
        group: &'static str,
        grant: KernelGrant,
    },
    #[error(
        "CLI command `{path}` registered by `{group}` resolved a descriptor holding `{grant}` \
         without declaring it"
    )]
    UndeclaredGrant {
        path: String,
        group: &'static str,
        grant: KernelGrant,
    },
}

/// Why a parsed command line could not be resolved to a registration.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error(transparent)]
    Parse(#[from] clap::Error),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}
