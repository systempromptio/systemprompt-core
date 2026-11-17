//! The provider check: every provided and slot-built value has one owner,
//! and a core member's requirement is met only under core standing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;
use std::collections::HashMap;

use systemprompt_identifiers::ExtensionId;

use super::error::CompositionError;
use super::{DeclaredMember, Standing};

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
