//! The composed binary's entry point and its one admission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Result;
use systemprompt_config::SecretsSources;
use systemprompt_extension::{Admitted, DeclaredCommands};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::Profile;
use systemprompt_models::BuildIdentity;
use systemprompt_traits::BoxedSource;

use super::dispatch::Invocation;
use super::{interrupt, run_with};
use crate::kernel::{
    CommandOwner, CommandRegistry, RegistryError, ResolvedCommand, kernel_commands,
};

/// Admits the composition: against the loaded profile, or, for a
/// database-only kernel command run with `--database-url` and no profile,
/// over every linked member with none disabled (`None`).
pub type AdmitFn = Box<dyn FnOnce(Option<&Profile>) -> Result<Admitted, BoxedSource> + Send>;

#[derive(Debug, thiserror::Error)]
pub enum CliRunError {
    #[error("composition admission failed")]
    Admit(#[source] BoxedSource),
    #[error("command {path:?} belongs to disabled extension {owner}")]
    CommandDisabled {
        path: Vec<&'static str>,
        owner: ExtensionId,
    },
    #[error("command {path:?} belongs to a member and requires a profile; it cannot run with --database-url alone")]
    ProfileRequired { path: Vec<&'static str> },
    #[error("command registry")]
    Registry(#[from] RegistryError),
    #[error("command failed")]
    Command(#[source] BoxedSource),
}

pub fn run_composed<A>(
    build: BuildIdentity,
    declared: DeclaredCommands,
    secrets: SecretsSources,
    admit: A,
) -> Result<ExitCode, CliRunError>
where
    A: FnOnce(Option<&Profile>) -> Result<Admitted, BoxedSource> + Send + 'static,
{
    let registry = CommandRegistry::build(&declared, &kernel_commands())?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|source| CliRunError::Command(Box::new(source)))?;
    let outcome = runtime.block_on(Box::pin(run_with(
        build,
        registry,
        secrets,
        Box::new(admit),
    )));
    match interrupt::exit_code(outcome) {
        Ok(code) => Ok(code),
        Err(error) => match error.downcast::<CliRunError>() {
            Ok(typed) => Err(typed),
            Err(other) => Err(CliRunError::Command(other.into_boxed_dyn_error())),
        },
    }
}

pub fn refuse_disabled(resolved: &ResolvedCommand, profile: &Profile) -> Result<(), CliRunError> {
    match &resolved.owner {
        CommandOwner::Member(owner) if profile.extensions.is_disabled(owner) => {
            Err(CliRunError::CommandDisabled {
                path: resolved.path.clone(),
                owner: owner.clone(),
            })
        },
        _ => Ok(()),
    }
}

pub(super) fn admit_for(
    invocation: &Invocation,
    profile: Option<&systemprompt_config::LoadedProfile>,
    admit: AdmitFn,
) -> Result<Option<Arc<Admitted>>> {
    let Some(profile) = profile.map(systemprompt_config::LoadedProfile::profile) else {
        return Ok(None);
    };
    if let Some(resolved) = invocation.resolved() {
        refuse_disabled(resolved, profile)?;
    }
    let admitted = admit(Some(profile)).map_err(CliRunError::Admit)?;
    Ok(Some(Arc::new(admitted)))
}

pub fn admit_unprofiled(
    registered: Option<&ResolvedCommand>,
    admit: AdmitFn,
) -> Result<Admitted, CliRunError> {
    if let Some(resolved) = registered {
        return Err(CliRunError::ProfileRequired {
            path: resolved.path.clone(),
        });
    }
    admit(None).map_err(CliRunError::Admit)
}
