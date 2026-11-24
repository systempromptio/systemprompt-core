//! The admission checks: removals and the members withheld with a removed,
//! disabled or withheld dependency, secrets kinds, member dependencies, job
//! names (one owner per name), disabled members, providers, CLI commands,
//! route sets and gateway routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;

#[cfg(feature = "cli")]
use systemprompt_extension::admitted::AdmittedCommand;
use systemprompt_extension::{AbsentDependency, DuplicateJobOwner, MemberWithheld};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::{DisabledExtension, ExtensionsConfig};

#[cfg(feature = "config")]
use super::SecretsSourceDecl;
use super::error::CompositionError;
use super::{DeclaredMember, Member, RESERVED_SECRETS_KINDS, SecretsKind};

pub(super) fn apply_removals(
    members: Vec<Member>,
    removed: &[TypeId],
) -> Result<(Vec<Member>, Vec<(TypeId, ExtensionId)>), CompositionError> {
    let mut kept = Vec::with_capacity(members.len());
    let mut gone = Vec::new();
    for member in members {
        if removed.contains(&member.type_id) {
            let meta = member.ext.metadata();
            let id =
                ExtensionId::try_new(meta.id).map_err(|source| CompositionError::InvalidId {
                    id: meta.id,
                    source,
                })?;
            if member.standing.required() {
                return Err(CompositionError::RequiredDisabled(id));
            }
            gone.push((member.type_id, id));
            continue;
        }
        kept.push(member);
    }
    Ok((kept, gone))
}

pub(super) fn withhold_dependants(
    members: &mut Vec<DeclaredMember>,
    absent: Vec<(TypeId, ExtensionId)>,
    because: AbsentDependency,
) -> Result<Vec<MemberWithheld>, CompositionError> {
    let mut absent = absent;
    let mut withheld = Vec::new();
    loop {
        let Some(position) = members.iter().position(|member| {
            member
                .sink
                .depends_on()
                .iter()
                .any(|dependency| absent.iter().any(|(ty, _)| *ty == dependency.type_id()))
        }) else {
            return Ok(withheld);
        };
        let member = members.remove(position);
        if member.standing.required() {
            return Err(CompositionError::RequiredDisabled(member.id));
        }
        let (dependency, reason) = member
            .sink
            .depends_on()
            .iter()
            .find_map(|dependency| {
                absent
                    .iter()
                    .find(|(ty, _)| *ty == dependency.type_id())
                    .map(|(_, id)| id.clone())
            })
            .map(|id| {
                let cascaded = withheld.iter().any(|w: &MemberWithheld| w.owner == id);
                let reason = if cascaded {
                    AbsentDependency::Withheld
                } else {
                    because
                };
                (id, reason)
            })
            .ok_or_else(|| CompositionError::RequiredDisabled(member.id.clone()))?;
        absent.push((member.type_id, member.id.clone()));
        withheld.push(MemberWithheld {
            owner: member.id,
            dependency,
            because: reason,
        });
    }
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

pub(super) fn granted_members(
    members: &[DeclaredMember],
    extensions: &ExtensionsConfig,
) -> Result<(), CompositionError> {
    extensions
        .secrets
        .keys()
        .find(|granted| !members.iter().any(|member| &member.id == *granted))
        .map_or(Ok(()), |unknown| {
            Err(CompositionError::UnknownSecretGrant(unknown.clone()))
        })
}

#[cfg(feature = "cli")]
const COMMAND_PARENTS: &[&[&str]] = &[
    &[],
    &["core"],
    &["core", "services"],
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
            for claim in decl.claims() {
                if !reserved.is_some_and(|reserved| conferred && reserved.claims.contains(claim)) {
                    return Err(CompositionError::ClaimNotConferred {
                        owner: member.id.clone(),
                        claim: *claim,
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
