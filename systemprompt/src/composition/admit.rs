//! The admission checks: removals, secrets kinds, member dependencies, job
//! names (one owner per name), disabled members, providers, CLI commands,
//! route sets and gateway routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;

#[cfg(feature = "cli")]
use systemprompt_extension::admitted::AdmittedCommand;
use systemprompt_extension::DuplicateJobOwner;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::DisabledExtension;

#[cfg(feature = "config")]
use super::SecretsSourceDecl;
use super::error::CompositionError;
use super::{DeclaredMember, Member, RESERVED_SECRETS_KINDS, SecretsKind};

pub(super) fn apply_removals(
    members: Vec<Member>,
    removed: &[TypeId],
) -> Result<Vec<Member>, CompositionError> {
    let mut kept = Vec::with_capacity(members.len());
    for member in members {
        if removed.contains(&member.type_id) {
            if member.standing.required() {
                let meta = member.ext.metadata();
                let id = ExtensionId::try_new(meta.id).map_err(|source| {
                    CompositionError::InvalidId {
                        id: meta.id,
                        source,
                    }
                })?;
                return Err(CompositionError::RequiredDisabled(id));
            }
            continue;
        }
        kept.push(member);
    }
    Ok(kept)
}

#[cfg(feature = "config")]
pub(super) fn check_secrets(
    secrets: Vec<SecretsSourceDecl>,
) -> Result<systemprompt_config::SecretsSources, CompositionError> {
    let mut seen: Vec<&'static str> = Vec::new();
    let mut out = Vec::with_capacity(secrets.len());
    for decl in secrets {
        let name = match decl.kind {
            SecretsKind::File => "file",
            SecretsKind::Env => "env",
            SecretsKind::Vault => "vault",
            SecretsKind::Install(name) => name,
        };
        if !matches!(decl.kind, SecretsKind::Install(_)) || RESERVED_SECRETS_KINDS.contains(&name) {
            return Err(CompositionError::ReservedSecretsKind(name));
        }
        if seen.contains(&name) {
            return Err(CompositionError::DuplicateSecretsKind(name));
        }
        seen.push(name);
        out.push(systemprompt_config::SecretsSourceRegistration {
            kind: name,
            factory: decl.factory,
        });
    }
    Ok(systemprompt_config::SecretsSources::new(out))
}

pub(super) fn check_member_dependencies(
    members: &[DeclaredMember],
) -> Result<(), CompositionError> {
    for member in members {
        for dependency in member.sink.depends_on() {
            let Some(target) = members
                .iter()
                .find(|other| other.type_id == dependency.type_id())
            else {
                return Err(CompositionError::Missing {
                    owner: member.id.clone(),
                    ty: dependency.type_name(),
                });
            };
            if target.standing.tag() != member.standing.tag() {
                return Err(CompositionError::CrossCapabilityDependency {
                    owner: member.id.clone(),
                    target: target.id.clone(),
                });
            }
        }
    }
    Ok(())
}

pub(super) fn check_job_names(members: &[DeclaredMember]) -> Result<(), CompositionError> {
    DuplicateJobOwner::refuse(members.iter().flat_map(|member| {
        member
            .sink
            .jobs()
            .iter()
            .map(move |job| (&member.id, job.job().name()))
    }))?;
    Ok(())
}

pub(super) fn disabled_members(
    members: &[DeclaredMember],
    disabled: &[DisabledExtension],
) -> Result<Vec<ExtensionId>, CompositionError> {
    for entry in disabled {
        let Some(member) = members.iter().find(|member| &member.id == entry.id()) else {
            return Err(CompositionError::UnknownExtension(entry.clone()));
        };
        if member.standing.required() {
            return Err(CompositionError::RequiredDisabled(member.id.clone()));
        }
    }
    Ok(disabled.iter().map(|entry| entry.id().clone()).collect())
}

#[cfg(feature = "cli")]
const COMMAND_PARENTS: &[&[&str]] = &[
    &[],
    &["core"],
    &["infra"],
    &["admin"],
    &["web"],
    &["plugins"],
];

#[cfg(feature = "cli")]
pub(super) fn admit_commands(
    members: &mut [DeclaredMember],
) -> Result<Vec<AdmittedCommand>, CompositionError> {
    let mut admitted: Vec<AdmittedCommand> = Vec::new();
    for member in members.iter_mut() {
        for decl in member.sink.take_commands() {
            let path = decl.path();
            let reserved = systemprompt_extension::reserved::RESERVED_CLI_PATHS
                .iter()
                .find(|reserved| reserved.path == path.as_slice());
            let conferred = member
                .standing
                .terms()
                .is_some_and(|terms| terms.cli().any(|cli| cli.path == path.as_slice()));
            if let Some(reserved) = reserved
                && !conferred
            {
                return Err(CompositionError::ReservedCliPath {
                    owner: member.id.clone(),
                    path: reserved.path,
                });
            }
            for grant in decl.grants() {
                if !reserved.is_some_and(|reserved| conferred && reserved.grants.contains(grant)) {
                    return Err(CompositionError::GrantNotConferred {
                        owner: member.id.clone(),
                        grant: *grant,
                    });
                }
            }
            if admitted.iter().any(|other| other.decl.path() == path) {
                return Err(CompositionError::CommandConflict { path });
            }
            admitted.push(AdmittedCommand {
                owner: member.id.clone(),
                core: member.standing.terms().and_then(|terms| terms.core),
                decl,
            });
        }
    }
    for command in &admitted {
        let parent = command.decl.parent();
        let provided = COMMAND_PARENTS.contains(&parent)
            || admitted
                .iter()
                .any(|other| other.decl.path().as_slice() == parent);
        if !provided {
            return Err(CompositionError::ParentMissing {
                leaf: command.decl.name(),
                parent,
            });
        }
    }
    Ok(admitted)
}
