//! The admission checks: removals, secrets kinds, member dependencies,
//! disabled members, providers, CLI commands, route sets and gateway routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;
use std::collections::{HashMap, HashSet};

use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES, PrivilegedShape};
use systemprompt_extension::{
    Admitted, ContributionError, DeclaredCommands, Extension, ExtensionConfigError, SlotError,
    StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_manifest::Profile;
use systemprompt_traits::authz::SubjectDimension;

#[cfg(feature = "cli")]
use systemprompt_extension::admitted::AdmittedCommand;
#[cfg(feature = "api")]
use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedRouteSet};
#[cfg(feature = "cli")]
use systemprompt_extension::cli::KernelGrant;
#[cfg(feature = "api")]
use systemprompt_extension::router::{Allowance, BasePath, BaseRef, RouteSet};

use super::error::CompositionError;
use super::{
    Conferral, CoreTerms, DeclaredMember, Member, RESERVED_SECRETS_KINDS, SecretsKind, Standing,
};
#[cfg(feature = "config")]
use super::SecretsSourceDecl;

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

pub(super) fn disabled_members(
    members: &[DeclaredMember],
    disabled: &[ExtensionId],
) -> Result<Vec<ExtensionId>, CompositionError> {
    for id in disabled {
        let Some(member) = members.iter().find(|member| &member.id == id) else {
            return Err(CompositionError::UnknownExtension(id.as_str().to_owned()));
        };
        if member.standing.required() {
            return Err(CompositionError::RequiredDisabled(member.id.clone()));
        }
    }
    Ok(disabled.to_vec())
}

pub(super) fn check_providers(
    enabled: &[DeclaredMember],
    dropped: &[DeclaredMember],
) -> Result<(), CompositionError> {
    let mut providers: HashMap<TypeId, ValueSource<'_>> = HashMap::new();
    for member in enabled {
        for entry in member.sink.provides() {
            let source = ValueSource {
                owner: &member.id,
                standing: member.standing,
                slot: false,
            };
            if let Some(first) = providers.insert(entry.type_id(), source) {
                return Err(CompositionError::DuplicateProvider {
                    ty: entry.type_name(),
                    first: first.owner.clone(),
                    second: member.id.clone(),
                });
            }
        }
    }
    for owned in slot_owners(enabled)? {
        let source = ValueSource {
            owner: owned.owner,
            standing: owned.standing,
            slot: true,
        };
        if let Some(first) = providers.insert(owned.built, source) {
            return Err(CompositionError::DuplicateProvider {
                ty: owned.name,
                first: first.owner.clone(),
                second: owned.owner.clone(),
            });
        }
    }
    for member in enabled {
        for requirement in member.sink.requires() {
            let ty = requirement.type_id();
            match providers.get(&ty) {
                Some(source) if matches!(source.standing, Standing::Install) => {
                    if let Some(terms) = member.standing.terms()
                        && (source.slot || !terms.substitutes(ty))
                    {
                        return Err(CompositionError::InstallSubstitution {
                            owner: member.id.clone(),
                            ty: requirement.type_name(),
                            provider: source.owner.clone(),
                        });
                    }
                },
                Some(_) => {},
                None => {
                    if let Some(provider) = dropped.iter().find(|other| {
                        other.sink.provides().iter().any(|p| p.type_id() == ty)
                            || other
                                .sink
                                .slots()
                                .iter()
                                .any(|slot| slot.built().type_id() == ty)
                    }) {
                        return Err(CompositionError::DisabledDependency {
                            owner: member.id.clone(),
                            ty: requirement.type_name(),
                            provider: provider.id.clone(),
                        });
                    }
                },
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ValueSource<'a> {
    owner: &'a ExtensionId,
    standing: Standing,
    slot: bool,
}

struct SlotOwner<'a> {
    slot: TypeId,
    built: TypeId,
    name: &'static str,
    owner: &'a ExtensionId,
    standing: Standing,
}

fn slot_owners(enabled: &[DeclaredMember]) -> Result<Vec<SlotOwner<'_>>, CompositionError> {
    let mut owners: Vec<SlotOwner<'_>> = Vec::new();
    for member in enabled {
        for record in member.sink.slots() {
            let built = record.built().type_id();
            match owners.iter_mut().find(|owned| owned.built == built) {
                Some(owned) if owned.slot != record.slot() => {
                    return Err(CompositionError::DuplicateProvider {
                        ty: record.built().type_name(),
                        first: owned.owner.clone(),
                        second: member.id.clone(),
                    });
                },
                Some(owned) => {
                    if matches!(owned.standing, Standing::Install)
                        && matches!(member.standing, Standing::Core(_))
                    {
                        owned.owner = &member.id;
                        owned.standing = member.standing;
                    }
                },
                None => owners.push(SlotOwner {
                    slot: record.slot(),
                    built,
                    name: record.built().type_name(),
                    owner: &member.id,
                    standing: member.standing,
                }),
            }
        }
    }
    Ok(owners)
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
