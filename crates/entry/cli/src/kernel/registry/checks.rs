//! The registry's build-time checks: where a kernel command may attach, no
//! duplicate path, and every reserved path, grant and claim re-checked.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::DeclaredCommands;
use systemprompt_extension::cli::CommandDecl;
use systemprompt_extension::reserved::RESERVED_CLI_PATHS;

use super::RegistryEntry;
use crate::kernel::registry_error::RegistryError;

pub(super) fn attaches_under_kernel_or(declared: &DeclaredCommands, decl: &CommandDecl) -> bool {
    decl.parent().len() < 2
        || declared
            .commands()
            .iter()
            .any(|command| command.decl.path() == decl.parent())
}

pub(super) fn refuse_duplicates(entries: &[RegistryEntry]) -> Result<(), RegistryError> {
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

pub(super) fn recheck_reserved(entries: &[RegistryEntry]) -> Result<(), RegistryError> {
    for entry in entries {
        let path = entry.decl.path();
        let reserved = RESERVED_CLI_PATHS
            .iter()
            .find(|reserved| reserved.path == path.as_slice());
        if let Some(reserved) = reserved
            && !entry.holds(reserved.owner)
        {
            return Err(RegistryError::ReservedPath {
                path: entry.path_text(),
                claimant: entry.owner.clone(),
                owner: reserved.owner,
            });
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
        if let Some(claim) = entry
            .decl
            .claims()
            .iter()
            .find(|claim| !reserved.is_some_and(|reserved| reserved.claims.contains(claim)))
        {
            return Err(RegistryError::ClaimNotListed {
                path: entry.path_text(),
                claimant: entry.owner.clone(),
                claim: *claim,
            });
        }
    }
    Ok(())
}
